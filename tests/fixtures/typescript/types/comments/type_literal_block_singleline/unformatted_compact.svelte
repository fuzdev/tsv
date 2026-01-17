<script lang="ts">
type A={a:A/* block */;b:B};
type B={a/* before colon */:A;b:B};
type C={a:/* after colon */A;b:B};
</script>
