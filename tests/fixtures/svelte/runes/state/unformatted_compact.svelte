<script lang="ts">
let counter = $state({count: 0});
let snapshot = $state.snapshot(counter);
let eager = $state.eager(counter.count);
let raw = $state.raw([1, 2, 3]);
</script>
