<script lang="ts">
// Mix of Fluid, NeverBreakAfterOperator, BreakAfterOperator in same scope

// Fluid pattern - call stays with =
const a=b.map((x)=>x.c);

// NeverBreakAfterOperator - stays inline
const d={e:true};

// BreakAfterOperator - long expression (exceeds 100 chars)
const longExpr=verylongvariable+anotherlongvariable+yetanotherlongvariable+morelongvariable;

// Assignment to destructured pattern with call
const {g}=h.filter((x)=>x.i);

// Assignment in for loop initializer
for(let j=k.map((x)=>x.l);j.length>0;j.pop()){fn(j);}
</script>
