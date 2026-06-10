<script lang="ts">
// The outer arrow's return type breaks the curried signatures onto their own
// lines, but an arrow inside the terminal block body (the object-property arrow
// here) is NOT part of the chain, so it stays hugged to `=>`.
const fn=(a:A):B=>()=>{return{x:()=>inner(value),y:obj.prop};};
</script>
