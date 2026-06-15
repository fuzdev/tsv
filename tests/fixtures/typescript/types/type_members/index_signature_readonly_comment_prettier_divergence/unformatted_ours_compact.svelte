<script lang="ts">
// Comment between `readonly` and `[` is preserved after the keyword; prettier
// relocates it inside the brackets before the key (see output_prettier.svelte).
interface I {
readonly/* c */[k:string]:A;
}
</script>
