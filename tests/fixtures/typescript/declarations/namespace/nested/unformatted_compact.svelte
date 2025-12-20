<script lang="ts">
// Dotted syntax
namespace A.B{export const a=1;}

namespace A.B.C{export const a=1;}

// Block-nested syntax
namespace D{namespace E{export const a=1;}}
</script>
