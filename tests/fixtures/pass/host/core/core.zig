// Copyright (c) 2026 The Billy Company
const std = @import("std");

pub const value: usize = @intFromBool(std.mem.eql(u8, "a", "a"));
