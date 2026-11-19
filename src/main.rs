#![no_std]
#![no_main]
#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

#[cfg(not(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
)))]
compile_error!("zdot currently supports Linux x86_64 and aarch64");

mod linux;
use core::ffi::CStr;
use core::fmt::Write;
use linux::{Fd, O_DIRECTORY, O_NOFOLLOW, O_PATH, Out, Path, Result};
use zclap::{Arg, Command, Event};
use zdot::{Entry, MAX_ENTRIES, MAX_PROFILE, Profile};

#[cfg(target_arch = "x86_64")]
core::arch::global_asm!(
    ".global _start",
    ".type _start,@function",
    "_start:",
    "xor rbp, rbp",
    "mov rdi, rsp",
    "and rsp, -16",
    "call zdot_start",
    "ud2",
);
#[cfg(target_arch = "aarch64")]
core::arch::global_asm!(
    ".global _start",
    ".type _start,%function",
    "_start:",
    "mov x0, sp",
    "bl zdot_start",
    "brk #0",
);

#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    linux::print(2, b"zdot: internal error\n");
    linux::exit(101)
}

const HELP: &str = "zdot — explicit dotfiles, small native core

Usage:
  zdot init --root REPO
  zdot plan|apply|status|remove --root REPO [--target HOME] [--profile FILE]
  zdot sync --root REPO status|pull|push

Options:
  --root PATH      Dotfile repository (default: current directory)
  --target PATH    Existing target directory (default: HOME)
  --profile FILE   Relative tab-separated manifest (default: profile.zdot)
  --state-dir PATH Absolute private journal directory (default: TARGET/.zdot-state)
  --backup         Move conflicts to the deployment journal (apply/plan only)
  --json           Machine-readable plan, status or result
  -h, --help       Show this help
  -V, --version    Show the version

Profiles contain source<TAB>destination, both relative. Blank lines and # comments
are ignored. Sources may be files or directories. Sources and parent directories
must not be symlinks. Apply refuses conflicts unless --backup is explicit.
Remove restores backups and refuses to remove replaced or edited destinations.
Git runs only for the explicit sync command; pull is always --ff-only.
";

const CLI: Command<'static> = Command::new("zdot").args(&[
    Arg::positional("command").one_of(&["init", "plan", "apply", "status", "remove", "sync"]),
    Arg::positional("operation").one_of(&["status", "pull", "push"]),
    Arg::value("root"),
    Arg::value("target"),
    Arg::value("profile"),
    Arg::value("state-dir"),
    Arg::flag("backup"),
    Arg::flag("json"),
    Arg::flag("help").short('h'),
    Arg::flag("version").short('V'),
]);

struct Options<'a> {
    command: &'a str,
    operation: &'a str,
    root: &'a str,
    target: &'a str,
    profile: &'a str,
    state_dir: &'a str,
    backup: bool,
    json: bool,
}

#[unsafe(no_mangle)]
unsafe extern "C" fn zdot_start(stack: *const usize) -> ! {
    let argc = unsafe { *stack };
    let argv = unsafe { stack.add(1) as *const *const u8 };
    let envp = unsafe { argv.add(argc + 1) };
    if argc > 128 {
        fail("too many arguments");
    }
    let mut args = [""; 128];
    for (i, arg) in args.iter_mut().enumerate().take(argc.saturating_sub(1)) {
        let ptr = unsafe { *argv.add(i + 1) };
        *arg = match unsafe { CStr::from_ptr(ptr.cast()) }.to_str() {
            Ok(s) => s,
            Err(_) => fail("arguments must be UTF-8"),
        };
    }
    let result = run(&args[..argc.saturating_sub(1)], envp);
    match result {
        Ok(code) => linux::exit(code),
        Err(message) => fail(message),
    }
}

fn fail(message: &str) -> ! {
    let _ = writeln!(Out(2), "zdot: {message}");
    linux::exit(2)
}

fn env<'a>(envp: *const *const u8, name: &str) -> Option<&'a str> {
    let mut ptr = envp;
    loop {
        let value = unsafe { *ptr };
        if value.is_null() {
            return None;
        }
        if let Ok(text) = unsafe { CStr::from_ptr(value.cast()) }.to_str()
            && let Some(tail) = text.strip_prefix(name).and_then(|s| s.strip_prefix('='))
        {
            return Some(tail);
        }
        ptr = unsafe { ptr.add(1) };
    }
}

fn run(args: &[&str], envp: *const *const u8) -> Result<i32> {
    let mut options = Options {
        command: "",
        operation: "",
        root: ".",
        target: env(envp, "HOME").unwrap_or(""),
        profile: "profile.zdot",
        state_dir: "",
        backup: false,
        json: false,
    };
    let mut parser = CLI.parse(args);
    while let Some(event) = parser
        .next()
        .map_err(|_| "invalid arguments; run zdot --help")?
    {
        match event {
            Event::Flag { arg: "help" } => {
                linux::print(1, HELP.as_bytes());
                return Ok(0);
            }
            Event::Flag { arg: "version" } => {
                linux::print(
                    1,
                    concat!("zdot ", env!("CARGO_PKG_VERSION"), "\n").as_bytes(),
                );
                return Ok(0);
            }
            Event::Flag { arg: "json" } => options.json = true,
            Event::Flag { arg: "backup" } => options.backup = true,
            Event::Value {
                arg: "command",
                value,
            } => options.command = value,
            Event::Value {
                arg: "operation",
                value,
            } => options.operation = value,
            Event::Value { arg: "root", value } => options.root = value,
            Event::Value {
                arg: "target",
                value,
            } => options.target = value,
            Event::Value {
                arg: "profile",
                value,
            } => options.profile = value,
            Event::Value {
                arg: "state-dir",
                value,
            } => {
                if value.is_empty() {
                    return Err("--state-dir must be an absolute path");
                }
                options.state_dir = value;
            }
            _ => {}
        }
    }
    parser
        .finish()
        .map_err(|_| "invalid arguments; run zdot --help")?;
    if options.command.is_empty() {
        linux::print(1, HELP.as_bytes());
        return Ok(0);
    }
    if options.command != "sync" && !options.operation.is_empty() {
        return Err("only sync accepts a second positional argument");
    }
    if options.backup && !matches!(options.command, "apply" | "plan") {
        return Err("--backup is only valid for apply or plan");
    }
    if !options.state_dir.is_empty() && matches!(options.command, "init" | "sync") {
        return Err("--state-dir is only valid for plan, apply, status or remove");
    }
    zdot::validate_relative(options.profile)?;
    if options.command == "init" {
        return init(&options);
    }
    let root = linux::directory(options.root)?;
    let root_path = linux::canonical(root.0)?;
    if options.command == "sync" {
        if options.json {
            return Err("--json is not available for Git output");
        }
        return sync(&options, &root_path, envp);
    }
    if options.target.is_empty() {
        return Err("--target or HOME is required");
    }
    let target = linux::directory(options.target)?;
    let target_path = linux::canonical(target.0)?;
    let storage = Storage::new(&options, root_path.as_str(), target_path.as_str())?;
    let mut owner = Path::from(root_path.as_str())?;
    owner.push("\n")?;
    owner.push(options.profile)?;
    owner.push("\n")?;
    if storage.explicit {
        owner.push(target_path.as_str())?;
        owner.push("\n")?;
    }
    let mut key = Path::new();
    write!(
        &mut key,
        "{:016x}",
        zdot::namespace(root_path.as_str(), options.profile)
    )
    .map_err(|_| "namespace overflow")?;
    let state = open_state(&storage, key.as_str(), owner.as_str())?;
    if options.command == "remove" {
        return remove(
            &options,
            root_path.as_str(),
            target.0,
            &storage,
            key.as_str(),
            owner.as_str(),
            state,
        );
    }
    let mut bytes = [0u8; MAX_PROFILE];
    let source_parent =
        parent(root.0, options.profile, false, &mut 0)?.ok_or("profile parent does not exist")?;
    let len = linux::read_file(source_parent.fd.0, source_parent.name.as_str(), &mut bytes)?;
    let text = core::str::from_utf8(&bytes[..len]).map_err(|_| "profile must be UTF-8")?;
    let profile = Profile::parse(text)?;
    storage.validate_profile(&profile, root_path.as_str(), target_path.as_str())?;
    let statuses = inspect(&profile, root.0, root_path.as_str(), target.0)?;
    if matches!(options.command, "plan" | "status") {
        output_plan(
            &options,
            root_path.as_str(),
            target_path.as_str(),
            storage.path.as_str(),
            &profile,
            &statuses,
        );
        return Ok(
            if options.command == "status"
                && statuses[..profile.entries().len()]
                    .iter()
                    .any(|s| *s != Status::Present)
            {
                3
            } else {
                0
            },
        );
    }
    apply(
        &options,
        root_path.as_str(),
        target.0,
        &storage,
        key.as_str(),
        owner.as_str(),
        state,
        text,
        &profile,
        &statuses,
    )
}

fn init(options: &Options<'_>) -> Result<i32> {
    match linux::mkdir(linux::AT_FDCWD, options.root, 0o755) {
        Ok(()) | Err(linux::EEXIST) => {}
        Err(_) => return Err("cannot create repository (parent directory must exist)"),
    }
    let root = linux::directory(options.root)?;
    let location = parent(root.0, options.profile, false, &mut 0)?
        .ok_or("profile parent directory must already exist")?;
    if linux::exists(location.fd.0, location.name.as_str())? {
        return Err("profile already exists; init never replaces it");
    }
    match linux::mkdir(root.0, "files", 0o755) {
        Ok(()) | Err(linux::EEXIST) => {}
        Err(_) => return Err("cannot create files directory"),
    }
    let _files = linux::openat(root.0, "files", O_DIRECTORY | O_NOFOLLOW, 0)
        .map_err(|_| "files must be a real directory")?;
    linux::create_file(location.fd.0, location.name.as_str(),
        b"# source relative to this repository<TAB>destination relative to target\n# files/zshrc\t.zshrc\n")?;
    linux::fsync(location.fd.0)?;
    result(options, "init", 1);
    Ok(0)
}

fn sync(options: &Options<'_>, root: &Path, envp: *const *const u8) -> Result<i32> {
    let op = match options.operation {
        "status" => c"status",
        "pull" => c"pull",
        "push" => c"push",
        _ => return Err("sync requires status, pull or push"),
    };
    let mut args = [core::ptr::null(); 8];
    args[0] = c"git".as_ptr().cast();
    args[1] = c"-C".as_ptr().cast();
    args[2] = root.as_ptr();
    args[3] = op.as_ptr().cast();
    if options.operation == "pull" {
        args[4] = c"--ff-only".as_ptr().cast();
    }
    if options.operation == "status" {
        args[4] = c"--short".as_ptr().cast();
    }
    unsafe {
        linux::exec_git(&args, envp)?;
    }
    Ok(0)
}

struct Parent {
    fd: Fd,
    name: Path,
}

fn parent(base: i32, path: &str, create: bool, created: &mut u64) -> Result<Option<Parent>> {
    let (prefix, leaf) = path.rsplit_once('/').unwrap_or(("", path));
    let mut current = linux::openat(base, ".", O_DIRECTORY | O_NOFOLLOW, 0)
        .map_err(|_| "cannot duplicate directory handle")?;
    if !prefix.is_empty() {
        for (index, component) in prefix.split('/').enumerate() {
            current = match linux::openat(current.0, component, O_DIRECTORY | O_NOFOLLOW, 0) {
                Ok(next) => next,
                Err(linux::ENOENT) if !create => return Ok(None),
                Err(linux::ENOENT) => {
                    match linux::mkdir(current.0, component, 0o755) {
                        Ok(()) => *created |= 1u64 << index,
                        Err(linux::EEXIST) => {}
                        Err(_) => return Err("cannot create destination parent"),
                    }
                    linux::openat(current.0, component, O_DIRECTORY | O_NOFOLLOW, 0)
                        .map_err(|_| "destination parent changed while opening it")?
                }
                Err(_) => {
                    return Err(
                        "parent is not an accessible real directory; symlink parents are forbidden",
                    );
                }
            };
        }
    }
    Ok(Some(Parent {
        fd: current,
        name: Path::from(leaf)?,
    }))
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Status {
    Missing,
    Present,
    Conflict,
}

fn source_path(root: &str, entry: &Entry<'_>) -> Result<Path> {
    let mut path = Path::from(root)?;
    path.join(entry.source)?;
    Ok(path)
}

fn destination_status(target: i32, entry: &Entry<'_>, source: &str) -> Result<Status> {
    let Some(p) = parent(target, entry.destination, false, &mut 0)? else {
        return Ok(Status::Missing);
    };
    match linux::openat(p.fd.0, p.name.as_str(), O_PATH | O_NOFOLLOW, 0) {
        Err(linux::ENOENT) => Ok(Status::Missing),
        Err(_) => Err("cannot inspect destination"),
        Ok(fd) => {
            if linux::kind(fd.0)? == 0o120000 {
                let link = linux::readlink(p.fd.0, p.name.as_str())
                    .map_err(|_| "cannot read destination link")?;
                if link.as_str() == source {
                    return Ok(Status::Present);
                }
            }
            Ok(Status::Conflict)
        }
    }
}

fn inspect(
    profile: &Profile<'_>,
    root: i32,
    root_path: &str,
    target: i32,
) -> Result<[Status; MAX_ENTRIES]> {
    let mut statuses = [Status::Missing; MAX_ENTRIES];
    let target_path = linux::canonical(target)?;
    for (i, entry) in profile.entries().iter().enumerate() {
        let p = parent(root, entry.source, false, &mut 0)?.ok_or("source parent does not exist")?;
        let source = linux::openat(p.fd.0, p.name.as_str(), O_PATH | O_NOFOLLOW, 0)
            .map_err(|_| "source does not exist")?;
        if !matches!(linux::kind(source.0)?, 0o100000 | 0o040000) {
            return Err("sources must be regular files or directories, never symlinks or devices");
        }
        let absolute = source_path(root_path, entry)?;
        let mut destination = Path::from(target_path.as_str())?;
        destination.join(entry.destination)?;
        let a = absolute.as_str();
        let b = destination.as_str();
        if a == b
            || a.strip_prefix(b).is_some_and(|x| x.starts_with('/'))
            || b.strip_prefix(a).is_some_and(|x| x.starts_with('/'))
        {
            return Err("source and destination must not contain one another");
        }
        statuses[i] = destination_status(target, entry, absolute.as_str())?;
    }
    Ok(statuses)
}

fn json_string(text: &str) {
    linux::print(1, b"\"");
    let mut start = 0;
    for (i, byte) in text.bytes().enumerate() {
        if byte == b'"' || byte == b'\\' || byte < 32 {
            linux::print(1, &text.as_bytes()[start..i]);
            match byte {
                b'"' => linux::print(1, b"\\\""),
                b'\\' => linux::print(1, b"\\\\"),
                b'\n' => linux::print(1, b"\\n"),
                b'\r' => linux::print(1, b"\\r"),
                b'\t' => linux::print(1, b"\\t"),
                c => {
                    let _ = write!(Out(1), "\\u{c:04x}");
                }
            }
            start = i + 1;
        }
    }
    linux::print(1, &text.as_bytes()[start..]);
    linux::print(1, b"\"");
}

fn output_plan(
    options: &Options<'_>,
    root: &str,
    target: &str,
    state_dir: &str,
    profile: &Profile<'_>,
    statuses: &[Status],
) {
    if options.json {
        linux::print(1, b"{\"schema\":\"zdot.plan.v1\",\"root\":");
        json_string(root);
        linux::print(1, b",\"target\":");
        json_string(target);
        linux::print(1, b",\"profile\":");
        json_string(options.profile);
        linux::print(1, b",\"state_dir\":");
        json_string(state_dir);
        linux::print(1, b",\"entries\":[");
    }
    for (i, entry) in profile.entries().iter().enumerate() {
        let action = match statuses[i] {
            Status::Missing => "link",
            Status::Present => "keep",
            Status::Conflict if options.backup => "backup-and-link",
            Status::Conflict => "conflict",
        };
        if options.json {
            if i > 0 {
                linux::print(1, b",");
            }
            linux::print(1, b"{\"source\":");
            json_string(entry.source);
            linux::print(1, b",\"destination\":");
            json_string(entry.destination);
            linux::print(1, b",\"action\":");
            json_string(action);
            linux::print(1, b"}");
        } else {
            let _ = writeln!(Out(1), "{action}\t{}\t{}", entry.source, entry.destination);
        }
    }
    if options.json {
        linux::print(1, b"]}\n");
    }
}

/// Explicit recovery stores leave the target's top level free of metadata.
/// Every path component uses O_NOFOLLOW; plan/status never create directories.
struct Storage {
    path: Path,
    explicit: bool,
}

fn contains(parent: &str, child: &str) -> bool {
    parent == "/"
        || parent == child
        || child
            .strip_prefix(parent)
            .is_some_and(|tail| tail.starts_with('/'))
}

fn overlaps(a: &str, b: &str) -> bool {
    contains(a, b) || contains(b, a)
}

impl Storage {
    fn new(options: &Options<'_>, root: &str, target: &str) -> Result<Self> {
        let explicit = !options.state_dir.is_empty();
        let path = if explicit {
            let relative = options
                .state_dir
                .strip_prefix('/')
                .ok_or("--state-dir must be an absolute path")?;
            zdot::validate_relative(relative)?;
            Path::from(options.state_dir)?
        } else {
            let mut value = Path::from(target)?;
            value.join(".zdot-state")?;
            value
        };
        if explicit && (overlaps(path.as_str(), root) || contains(path.as_str(), target)) {
            return Err("state directory must not overlap the repository or contain the target");
        }
        Ok(Self { path, explicit })
    }

    fn open(&self, create: bool) -> Result<Option<Fd>> {
        let mut directory = linux::directory("/")?;
        for part in self.path.as_str()[1..].split('/') {
            directory = match linux::openat(directory.0, part, O_DIRECTORY | O_NOFOLLOW, 0) {
                Ok(fd) => fd,
                Err(linux::ENOENT) if !create => return Ok(None),
                Err(linux::ENOENT) => {
                    match linux::mkdir(directory.0, part, 0o700) {
                        Ok(()) => linux::fsync(directory.0)?,
                        Err(linux::EEXIST) => {}
                        Err(_) => return Err("cannot create private state directory"),
                    }
                    linux::openat(directory.0, part, O_DIRECTORY | O_NOFOLLOW, 0)
                        .map_err(|_| "state directory changed while opening it")?
                }
                Err(_) => return Err("state path must contain only accessible real directories"),
            };
        }
        linux::private_directory(directory.0)?;
        Ok(Some(directory))
    }

    fn validate_profile(&self, profile: &Profile<'_>, root: &str, target: &str) -> Result<()> {
        for entry in profile.entries() {
            let source = source_path(root, entry)?;
            let mut destination = Path::from(target)?;
            destination.join(entry.destination)?;
            if overlaps(self.path.as_str(), source.as_str())
                || overlaps(self.path.as_str(), destination.as_str())
            {
                return Err("state directory must not overlap any source or destination");
            }
        }
        Ok(())
    }
}

fn lock_target(target: i32, storage: &Storage) -> Result<(Fd, Option<Fd>)> {
    // Locking the directory coordinates different stores without a new dotfile.
    let directory = linux::lock_directory(target)?;
    // Default mode also keeps the lock contract used by older zdot binaries.
    let legacy = if !storage.explicit || linux::exists(target, ".zdot.lock")? {
        Some(linux::lock(target)?)
    } else {
        None
    };
    Ok((directory, legacy))
}

struct State {
    directory: Fd,
    backups: Fd,
    complete: bool,
}

fn open_state(storage: &Storage, key: &str, owner: &str) -> Result<Option<State>> {
    let Some(base) = storage.open(false)? else {
        return Ok(None);
    };
    let directory = match linux::openat(base.0, key, O_DIRECTORY | O_NOFOLLOW, 0) {
        Ok(fd) => fd,
        Err(linux::ENOENT) => return Ok(None),
        Err(_) => return Err("deployment journal is not a real directory"),
    };
    linux::private_directory(directory.0)?;
    let mut bytes = [0; 8192];
    let n = linux::read_file(directory.0, "owner", &mut bytes)?;
    if &bytes[..n] != owner.as_bytes() {
        return Err("deployment owner mismatch; refusing to use another journal");
    }
    let backups = linux::openat(directory.0, "backups", O_DIRECTORY | O_NOFOLLOW, 0)
        .map_err(|_| "deployment backup directory is missing or invalid")?;
    linux::private_directory(backups.0)?;
    let complete = linux::exists(directory.0, "complete")?;
    Ok(Some(State {
        directory,
        backups,
        complete,
    }))
}

fn create_state(storage: &Storage, key: &str, owner: &str, text: &str) -> Result<State> {
    let base = storage.open(true)?.ok_or("cannot create state directory")?;
    linux::mkdir(base.0, key, 0o700)
        .map_err(|_| "deployment journal already exists or cannot be created")?;
    let directory = linux::openat(base.0, key, O_DIRECTORY | O_NOFOLLOW, 0)
        .map_err(|_| "cannot open new journal")?;
    let written = (|| {
        linux::create_file(directory.0, "owner", owner.as_bytes())?;
        linux::create_file(directory.0, "profile", text.as_bytes())?;
        linux::mkdir(directory.0, "backups", 0o700)
            .map_err(|_| "cannot create backup directory")?;
        linux::fsync(directory.0)?;
        linux::fsync(base.0)?;
        Ok(())
    })();
    if let Err(error) = written {
        let _ = linux::unlink(directory.0, "owner", false);
        let _ = linux::unlink(directory.0, "profile", false);
        let _ = linux::unlink(directory.0, "backups", true);
        let _ = linux::unlink(base.0, key, true);
        return Err(error);
    }
    let backups = linux::openat(directory.0, "backups", O_DIRECTORY | O_NOFOLLOW, 0)
        .map_err(|_| "cannot open backup directory")?;
    Ok(State {
        directory,
        backups,
        complete: false,
    })
}

fn backup_name(index: usize) -> Path {
    let mut result = Path::new();
    let _ = write!(&mut result, "{index:03}");
    result
}

#[allow(clippy::too_many_arguments)]
fn apply(
    options: &Options<'_>,
    root: &str,
    target: i32,
    storage: &Storage,
    key: &str,
    owner: &str,
    state: Option<State>,
    text: &str,
    profile: &Profile<'_>,
    statuses: &[Status],
) -> Result<i32> {
    for (entry, status) in profile.entries().iter().zip(statuses) {
        if *status == Status::Conflict && !options.backup {
            let _ = writeln!(Out(2), "conflict\t{}", entry.destination);
            return Err(
                "destination conflict; nothing deployed (use --backup explicitly to preserve it)",
            );
        }
    }
    if let Some(state) = &state {
        let mut old = [0; MAX_PROFILE];
        let len = linux::read_file(state.directory.0, "profile", &mut old)?;
        if &old[..len] != text.as_bytes() {
            return Err("profile changed; remove the previous deployment before applying it");
        }
        if !state.complete {
            return Err("interrupted deployment; use remove to recover before applying again");
        }
        for (i, status) in statuses.iter().enumerate().take(profile.entries().len()) {
            if *status == Status::Conflict
                && linux::exists(state.backups.0, backup_name(i).as_str())?
            {
                return Err(
                    "a preserved original already exists; resolve the changed destination before removing this deployment",
                );
            }
        }
    }
    let _lock = lock_target(target, storage)?;
    // Repeat the read-only preflight after acquiring our cooperative lock.
    for (i, entry) in profile.entries().iter().enumerate() {
        let source = source_path(root, entry)?;
        if destination_status(target, entry, source.as_str())? != statuses[i] {
            return Err("destination changed since preflight; retry the command");
        }
    }
    let current_state = open_state(storage, key, owner)?;
    if current_state.is_some() != state.is_some() {
        return Err("deployment journal changed since preflight; retry");
    }
    if let Some(current) = &current_state {
        let mut bytes = [0; MAX_PROFILE];
        let len = linux::read_file(current.directory.0, "profile", &mut bytes)?;
        if !current.complete || &bytes[..len] != text.as_bytes() {
            return Err("deployment journal changed since preflight; retry");
        }
    }
    let new_state = current_state.is_none();
    let state = match current_state {
        Some(state) => state,
        None => create_state(storage, key, owner, text)?,
    };
    let mut linked = [false; MAX_ENTRIES];
    let mut moved = [false; MAX_ENTRIES];
    let mut created_dirs = [0u64; MAX_ENTRIES];
    let changes = (|| {
        let mut changed = 0;
        for (i, entry) in profile.entries().iter().enumerate() {
            if statuses[i] == Status::Present {
                continue;
            }
            let source = source_path(root, entry)?;
            let p = parent(target, entry.destination, true, &mut created_dirs[i])?
                .ok_or("cannot create destination parent")?;
            if statuses[i] == Status::Conflict {
                linux::rename(
                    p.fd.0,
                    p.name.as_str(),
                    state.backups.0,
                    backup_name(i).as_str(),
                )?;
                moved[i] = true;
                linux::fsync(state.backups.0)?;
                linux::fsync(p.fd.0)?;
            }
            linux::symlink(source.as_str(), p.fd.0, p.name.as_str())?;
            linked[i] = true;
            changed += 1;
            linux::fsync(p.fd.0)?;
        }
        if !state.complete {
            linux::create_file(state.directory.0, "complete", b"")?;
        }
        linux::fsync(state.directory.0)?;
        Ok(changed)
    })();
    match changes {
        Ok(changed) => {
            result(options, "apply", changed);
            Ok(0)
        }
        Err(error) => {
            let mut clean = true;
            for (i, entry) in profile.entries().iter().enumerate().rev() {
                let source = source_path(root, entry)?;
                if let Some(p) = parent(target, entry.destination, false, &mut 0)? {
                    if linked[i] {
                        if destination_status(target, entry, source.as_str())? == Status::Present {
                            clean &= linux::unlink(p.fd.0, p.name.as_str(), false).is_ok();
                        } else {
                            clean = false;
                        }
                    }
                    if moved[i] {
                        clean &= linux::rename(
                            state.backups.0,
                            backup_name(i).as_str(),
                            p.fd.0,
                            p.name.as_str(),
                        )
                        .is_ok();
                    }
                } else if linked[i] || moved[i] {
                    clean = false;
                }
                cleanup_dirs(target, entry.destination, created_dirs[i]);
            }
            if clean && new_state {
                let _ = clear_state(storage, key, &state);
            }
            if !clean {
                return Err(
                    "apply interrupted; journal retained, inspect destinations and run remove to recover",
                );
            }
            Err(error)
        }
    }
}

fn cleanup_dirs(target: i32, path: &str, mask: u64) {
    for index in (0..64).rev() {
        if mask & (1u64 << index) == 0 {
            continue;
        }
        let Some((end, _)) = path.match_indices('/').nth(index) else {
            continue;
        };
        if let Ok(Some(p)) = parent(target, &path[..end], false, &mut 0) {
            let _ = linux::unlink(p.fd.0, p.name.as_str(), true);
        }
    }
}

fn remove(
    options: &Options<'_>,
    root: &str,
    target: i32,
    storage: &Storage,
    key: &str,
    owner: &str,
    state: Option<State>,
) -> Result<i32> {
    let Some(state) = state else {
        result(options, "remove", 0);
        return Ok(0);
    };
    let mut bytes = [0; MAX_PROFILE];
    let len = linux::read_file(state.directory.0, "profile", &mut bytes)?;
    let text = core::str::from_utf8(&bytes[..len]).map_err(|_| "journal profile is not UTF-8")?;
    let profile = Profile::parse(text)?;
    storage.validate_profile(&profile, root, linux::canonical(target)?.as_str())?;
    let mut statuses = [Status::Missing; MAX_ENTRIES];
    let mut backup = [false; MAX_ENTRIES];
    for (i, entry) in profile.entries().iter().enumerate() {
        let source = source_path(root, entry)?;
        statuses[i] = destination_status(target, entry, source.as_str())?;
        backup[i] = linux::exists(state.backups.0, backup_name(i).as_str())?;
        if statuses[i] == Status::Conflict && (state.complete || backup[i]) {
            let _ = writeln!(Out(2), "changed\t{}", entry.destination);
            return Err("destination was replaced; remove preserves it and all other mappings");
        }
    }
    let _lock = lock_target(target, storage)?;
    for (i, entry) in profile.entries().iter().enumerate() {
        let source = source_path(root, entry)?;
        if destination_status(target, entry, source.as_str())? != statuses[i] {
            return Err("destination changed since preflight; retry the command");
        }
    }
    let current_state = open_state(storage, key, owner)?
        .ok_or("deployment journal changed since preflight; retry")?;
    let mut current_bytes = [0; MAX_PROFILE];
    let current_len = linux::read_file(current_state.directory.0, "profile", &mut current_bytes)?;
    if current_state.complete != state.complete || &current_bytes[..current_len] != text.as_bytes()
    {
        return Err("deployment journal changed since preflight; retry");
    }
    for (i, expected) in backup.iter().enumerate().take(profile.entries().len()) {
        if linux::exists(current_state.backups.0, backup_name(i).as_str())? != *expected {
            return Err("deployment backups changed since preflight; retry");
        }
    }
    let state = current_state;
    let mut removed = [false; MAX_ENTRIES];
    let mut restored = [false; MAX_ENTRIES];
    let changes = (|| {
        let mut changed = 0;
        for (i, entry) in profile.entries().iter().enumerate() {
            if statuses[i] == Status::Present || backup[i] {
                let p = parent(target, entry.destination, backup[i], &mut 0)?
                    .ok_or("destination parent disappeared")?;
                if statuses[i] == Status::Present {
                    let source = source_path(root, entry)?;
                    if destination_status(target, entry, source.as_str())? != Status::Present {
                        return Err("destination changed during removal");
                    }
                    linux::unlink(p.fd.0, p.name.as_str(), false)?;
                    removed[i] = true;
                    changed += 1;
                }
                if backup[i] {
                    linux::rename(
                        state.backups.0,
                        backup_name(i).as_str(),
                        p.fd.0,
                        p.name.as_str(),
                    )?;
                    restored[i] = true;
                }
                linux::fsync(p.fd.0)?;
            }
        }
        Ok(changed)
    })();
    if let Err(error) = changes {
        let mut clean = true;
        for (i, entry) in profile.entries().iter().enumerate().rev() {
            if let Some(p) = parent(target, entry.destination, false, &mut 0)? {
                if restored[i] {
                    clean &= linux::rename(
                        p.fd.0,
                        p.name.as_str(),
                        state.backups.0,
                        backup_name(i).as_str(),
                    )
                    .is_ok();
                }
                if removed[i] {
                    clean &=
                        linux::symlink(source_path(root, entry)?.as_str(), p.fd.0, p.name.as_str())
                            .is_ok();
                }
            } else if restored[i] || removed[i] {
                clean = false;
            }
        }
        return Err(if clean {
            error
        } else {
            "remove interrupted; journal retained for recovery"
        });
    }
    clear_state(storage, key, &state)?;
    result(options, "remove", changes?);
    Ok(0)
}

fn clear_state(storage: &Storage, key: &str, state: &State) -> Result<()> {
    // Check backups first. Nonempty backups are never discarded.
    linux::unlink(state.directory.0, "backups", true)?;
    linux::unlink(state.directory.0, "complete", false)?;
    linux::unlink(state.directory.0, "profile", false)?;
    linux::unlink(state.directory.0, "owner", false)?;
    let base = storage.open(false)?.ok_or("journal parent disappeared")?;
    linux::unlink(base.0, key, true)?;
    linux::fsync(base.0)?;
    // Keep the empty store available for later deployments.
    Ok(())
}

fn result(options: &Options<'_>, operation: &str, changed: usize) {
    if options.json {
        let _ = writeln!(
            Out(1),
            "{{\"schema\":\"zdot.result.v1\",\"operation\":\"{operation}\",\"changed\":{changed}}}"
        );
    } else {
        let _ = writeln!(Out(1), "{operation}: {changed} mapping(s) changed");
    }
}
