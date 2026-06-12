<script lang="ts">
// This comment has an apostrophe: isn't
const x = 1;

const f = (a: A, b = x, // isn't
): B => a;
</script>
