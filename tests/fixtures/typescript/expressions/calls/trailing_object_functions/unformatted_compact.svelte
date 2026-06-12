<script>
// Head args stay inline, trailing object breaks when it contains functions
fn('a','b',{prop:'value',fn1:async(a,b,c)=>{return a+b}});

// Multiple function properties
fn('a','b',{fn1:async()=>'',fn2:async(a,b,c)=>{return a}});
</script>
