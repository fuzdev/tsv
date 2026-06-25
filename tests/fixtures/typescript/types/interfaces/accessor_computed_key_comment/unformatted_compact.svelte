<script lang="ts">
// Comment inside a computed accessor key is preserved once (not duplicated onto the keyword)
interface I{get[/* c1 */a]():A;set[/* c2 */b](v:A);}
</script>
