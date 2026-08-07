<script lang="ts">
// Minimal async generic arrow: prettier forces `<T,>` on the single
// unconstrained type param (output_prettier), tsv emits the bare `<T>`.
const f=async <T>(x:T):Promise<T>=>x;
</script>
