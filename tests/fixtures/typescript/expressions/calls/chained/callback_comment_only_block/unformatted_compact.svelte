<script>
// Comment-only block body - detected as breaking, chain expands
aaaaaaaaaaaaa.bbbbbbbbbbbbbbb((x) => (a = new x.B())).ccccccccccccc(() => {
// comment
});

// import() counted as call node - chain has 3 calls, expands
import('a').then((x) => (a = new x.B())).catch(() => {
// comment
});

// Short chain with comment-only block - stays inline
a.b(() => {
// comment
});
</script>
