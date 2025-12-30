<script lang="ts">
let n = $state(0);

$inspect(n);
$inspect(n).with(console.log);

$effect(() => {
$inspect.trace('my effect');
n;
});
</script>
