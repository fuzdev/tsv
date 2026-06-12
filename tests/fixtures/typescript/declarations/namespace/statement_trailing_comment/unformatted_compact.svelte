<script lang="ts">
namespace A {
const a = 1; // line trailing
const b = 2; /* block trailing */
}

namespace B {
namespace C {} // trailing after nested namespace
const d = 3; // another
}
</script>
