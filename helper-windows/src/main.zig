const std = @import("std");

const windows = std.os.windows;
const HANDLE = windows.HANDLE;
const DWORD = windows.DWORD;
const BOOL = windows.BOOL;
const LPWSTR = windows.LPWSTR;
const LPCWSTR = windows.LPCWSTR;

// Win32 Structs
const JOBOBJECT_BASIC_PROCESS_ID_LIST = struct {
    NumberOfAssignedProcesses: DWORD,
    NumberOfProcessIdsInList: DWORD,
    ProcessIdList: [1024]usize,
};

const JobObjectBasicProcessIdList = 3;

extern "kernel32" fn CreateJobObjectW(lpJobAttributes: ?*anyopaque, lpName: ?LPCWSTR) callconv(std.os.windows.WINAPI) ?HANDLE;
extern "kernel32" fn AssignProcessToJobObject(hJob: HANDLE, hProcess: HANDLE) callconv(std.os.windows.WINAPI) BOOL;
extern "kernel32" fn QueryInformationJobObject(hJob: HANDLE, JobObjectInformationClass: u32, lpJobObjectInformation: *anyopaque, cbJobObjectInformation: DWORD, lpReturnLength: ?*DWORD) callconv(std.os.windows.WINAPI) BOOL;
extern "kernel32" fn CreateProcessW(
    lpApplicationName: ?LPCWSTR,
    lpCommandLine: LPWSTR,
    lpProcessAttributes: ?*anyopaque,
    lpThreadAttributes: ?*anyopaque,
    bInheritHandles: BOOL,
    dwCreationFlags: DWORD,
    lpEnvironment: ?*anyopaque,
    lpCurrentDirectory: ?LPCWSTR,
    lpStartupInfo: *anyopaque,
    lpProcessInformation: *anyopaque,
) callconv(std.os.windows.WINAPI) BOOL;
extern "kernel32" fn ResumeThread(hThread: HANDLE) callconv(std.os.windows.WINAPI) DWORD;
extern "kernel32" fn Sleep(dwMilliseconds: DWORD) callconv(std.os.windows.WINAPI) void;


const PROCESS_INFORMATION = struct {
    hProcess: HANDLE,
    hThread: HANDLE,
    dwProcessId: DWORD,
    dwThreadId: DWORD,
};

const STARTUPINFOW = struct {
    cb: DWORD = @sizeOf(STARTUPINFOW),
    lpReserved: ?LPWSTR = null,
    lpDesktop: ?LPWSTR = null,
    lpTitle: ?LPWSTR = null,
    dwX: DWORD = 0,
    dwY: DWORD = 0,
    dwXSize: DWORD = 0,
    dwYSize: DWORD = 0,
    dwXCountChars: DWORD = 0,
    dwYCountChars: DWORD = 0,
    dwFillAttribute: DWORD = 0,
    dwFlags: DWORD = 0,
    wShowWindow: u16 = 0,
    cbReserved2: u16 = 0,
    lpReserved2: ?*u8 = null,
    hStdInput: ?HANDLE = null,
    hStdOutput: ?HANDLE = null,
    hStdError: ?HANDLE = null,
};

const CREATE_SUSPENDED = 0x00000004;
const INFINITE = 0xFFFFFFFF;

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer _ = gpa.deinit();
    const allocator = gpa.allocator();

    // Get command line arguments
    const args = try std.process.argsAlloc(allocator);
    defer std.process.argsFree(allocator, args);

    if (args.len < 2) {
        std.debug.print("Usage: grapevine-helper.exe <path_to_game_exe> [game_args...]\n", .{});
        std.process.exit(1);
    }

    const game_path = args[1];

    // Reconstruct cmdline for CreateProcessW
    var cmdline_list = std.ArrayList(u8).init(allocator);
    defer cmdline_list.deinit();

    // Wrap executable path in quotes
    try cmdline_list.append('"');
    try cmdline_list.appendSlice(game_path);
    try cmdline_list.append('"');

    for (args[2..]) |arg| {
        try cmdline_list.append(' ');
        try cmdline_list.appendSlice(arg);
    }
    try cmdline_list.append(0); // Null terminator

    // Convert to UTF-16
    const cmdline_w = try std.unicode.utf8ToUtf16LeWithNull(allocator, cmdline_list.items[0..cmdline_list.items.len-1]);
    defer allocator.free(cmdline_w);

    const game_dir = std.fs.path.dirname(game_path);
    var game_dir_w: ?[:0]const u16 = null;
    if (game_dir) |dir| {
        game_dir_w = try std.unicode.utf8ToUtf16LeWithNull(allocator, dir);
    }
    defer if (game_dir_w) |w| allocator.free(w);

    // Create Job Object
    const job_handle = CreateJobObjectW(null, null) orelse {
        std.debug.print("Failed to create Job Object\n", .{});
        std.process.exit(1);
    };
    defer _ = windows.CloseHandle(job_handle);

    // Prepare startup structures
    var si = STARTUPINFOW{};
    var pi = PROCESS_INFORMATION{
        .hProcess = undefined,
        .hThread = undefined,
        .dwProcessId = 0,
        .dwThreadId = 0,
    };

    // Spawn process suspended so we can assign it to the job object first
    const create_res = CreateProcessW(
        null,
        cmdline_w,
        null,
        null,
        windows.FALSE,
        CREATE_SUSPENDED,
        null,
        if (game_dir_w) |dir| dir else null,
        &si,
        &pi,
    );

    if (create_res == windows.FALSE) {
        std.debug.print("Failed to launch game process\n", .{});
        std.process.exit(1);
    }
    defer _ = windows.CloseHandle(pi.hProcess);
    defer _ = windows.CloseHandle(pi.hThread);

    // Assign process to Job Object
    if (AssignProcessToJobObject(job_handle, pi.hProcess) == windows.FALSE) {
        std.debug.print("Failed to assign process to Job Object\n", .{});
        windows.TerminateProcess(pi.hProcess, 1) catch {};
        std.process.exit(1);
    }

    // Resume process execution
    _ = ResumeThread(pi.hThread);

    // Monitor loop
    const start_time = std.time.milliTimestamp();
    const status_path = "C:\\grapevine-status.json";

    while (true) {
        // Query process list in Job Object
        var id_list: JOBOBJECT_BASIC_PROCESS_ID_LIST = undefined;
        var ret_len: DWORD = 0;
        const query_res = QueryInformationJobObject(
            job_handle,
            JobObjectBasicProcessIdList,
            &id_list,
            @sizeOf(JOBOBJECT_BASIC_PROCESS_ID_LIST),
            &ret_len,
        );

        const active_count = if (query_res == windows.TRUE) id_list.NumberOfAssignedProcesses else 1;

        const current_time = std.time.milliTimestamp();
        const elapsed_sec = @divTrunc(current_time - start_time, 1000);

        // Write status file
        writeStatusFile(status_path, active_count, elapsed_sec) catch {};

        // If no processes remain in the Job Object, exit
        if (query_res == windows.TRUE and active_count == 0) {
            break;
        }

        // Sleep for 1 second
        Sleep(1000);
    }

    // Write final status file
    const end_time = std.time.milliTimestamp();
    const final_elapsed = @divTrunc(end_time - start_time, 1000);
    writeFinalStatusFile(status_path, final_elapsed) catch {};
}

fn writeStatusFile(path: []const u8, active_processes: u32, elapsed_seconds: i64) !void {
    const file = try std.fs.createFileAbsolute(path, .{});
    defer file.close();

    var buf: [256]u8 = undefined;
    const json = try std.fmt.bufPrint(&buf,
        \\{{
        \\  "status": "running",
        \\  "active_processes": {},
        \\  "elapsed_seconds": {}
        \\}}
    , .{ active_processes, elapsed_seconds });

    try file.writeAll(json);
}

fn writeFinalStatusFile(path: []const u8, elapsed_seconds: i64) !void {
    const file = try std.fs.createFileAbsolute(path, .{});
    defer file.close();

    var buf: [256]u8 = undefined;
    const json = try std.fmt.bufPrint(&buf,
        \\{{
        \\  "status": "finished",
        \\  "active_processes": 0,
        \\  "elapsed_seconds": {}
        \\}}
    , .{elapsed_seconds});

    try file.writeAll(json);
}
