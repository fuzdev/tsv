<script lang="ts">
// Block-body callback with trailing .catch() - stays on }) line
const a = await fn([1],3,async () => {throw a;}).catch((e: unknown) => e);

// Block-body callback with trailing .then() - stays on }) line
const b = await fn(a,b,() => {return a;}).then((x) => x + 1);

// Expression-body callbacks only - no breaking, stays flat
const c = items.map((x) => x.a).filter((x) => x > 0);

// Short chain with non-empty block and trailing member+call
const d = await fn(a,b,async () => {throw a;}).catch((e: unknown) => e);
</script>
