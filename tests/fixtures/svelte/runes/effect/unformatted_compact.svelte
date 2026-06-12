<script lang="ts">
let n = $state(0);

$effect(() => {
console.log(n);
});

$effect(() => {
console.log(n);

return () => {
console.log('cleanup');
};
});

$effect.pre(() => {
console.log('pre', n);
});

$effect(() => {
if ($effect.tracking()) {
console.log('tracking');
}
});

const fn = $effect.root(() => {
$effect(() => {
console.log(n);
});
return () => console.log('cleanup');
});

$effect(() => {
const val = $effect.pending();
console.log('pending:', val);
});
</script>
