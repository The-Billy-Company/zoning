// A build script, reaching where no module file may: out of the package entirely.
// Judged, this would be an `escape` violation — which is the point. A build graph's
// dependencies are not the module's, and the file that declares a package does not
// belong to it.
const elsewhere = @import("../../outside.zig");
const borrowed = @import("borrowed");
