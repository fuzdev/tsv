<script lang="ts">
let count = $state(0);

$effect(() => {
console.log(count);
});

$effect(() => {
console.log(count);

return () => {
console.log('cleanup');
};
});

$effect.pre(() => {
console.log('pre', count);
});

$effect(() => {
if ($effect.tracking()) {
console.log('tracking');
}
});

const cleanup = $effect.root(() => {
$effect(() => {
console.log(count);
});
return () => console.log('cleanup');
});

$effect(() => {
const pending = $effect.pending();
console.log('pending:', pending);
});
</script>
