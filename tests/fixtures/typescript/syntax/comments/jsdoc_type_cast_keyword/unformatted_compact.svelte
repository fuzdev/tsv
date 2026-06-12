<script>
// await with JSDoc comment in stripped grouping parens
async function fn1(){const a=await/** @type {Promise<A>} */expr;}

// yield with JSDoc comment in stripped grouping parens
function* fn2(){const b=yield/** @type {A} */expr;}
</script>
