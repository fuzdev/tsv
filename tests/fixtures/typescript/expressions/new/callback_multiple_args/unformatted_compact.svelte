<script lang="ts">
// Multiple arrow args force break
new Cls(()=>a,()=>b,()=>c)

// With type args
new Cls<T>(()=>a,()=>b,()=>c)

// Mixed with simple args
new Cls(x,()=>a,()=>b)

// Multiple function expressions
new Cls(function(){return a},function(){return b})

// Mixed arrow + function
new Cls(()=>a,function(){return b})
</script>
