const std = @import("std");

const win_libs = [_][]const u8{
    "kernel32",
    "user32",
    "gdi32",
    "comctl32",
    "imm32",
    "ole32",
    "windowscodecs",
};

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    const exe_mod = b.createModule(.{
        .root_source_file = b.path("src/main.zig"),
        .target = target,
        .optimize = optimize,
        .link_libc = true,
        .strip = true,
    });
    const exe = b.addExecutable(.{
        .name = "mascot",
        .root_module = exe_mod,
    });
    exe.subsystem = .Windows;
    exe.want_lto = true;
    exe.win32_manifest = b.path("app.manifest");
    for (win_libs) |lib| exe.root_module.linkSystemLibrary(lib, .{});
    b.installArtifact(exe);

    const test_mod = b.createModule(.{
        .root_source_file = b.path("src/main.zig"),
        .target = target,
        .optimize = optimize,
        .link_libc = true,
    });
    const unit_tests = b.addTest(.{
        .root_module = test_mod,
    });
    for (win_libs) |lib| unit_tests.root_module.linkSystemLibrary(lib, .{});
    const run_tests = b.addRunArtifact(unit_tests);
    const test_step = b.step("test", "Run unit tests");
    test_step.dependOn(&run_tests.step);
}
