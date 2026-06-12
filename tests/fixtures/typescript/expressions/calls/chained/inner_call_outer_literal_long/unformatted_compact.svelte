<script>
// 100 chars - fits on one line
fn1(fn2(arg)).fn3('aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa');

// 101 chars - outer call breaks, inner call stays inline
fn1(fn2(arg)).fn3('aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa');
</script>
