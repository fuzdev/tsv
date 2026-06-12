<script lang="ts">
// Short - spread with a logical chain fits inline (no break)
const short={method:verb,...(cond1&&cond2&&{body:data})};

// Boundary: spread chain line exactly 100 - chain stays inline
const fit={method:verb,...(conditionAaaaaaaaaaaaaaaa!=='GET'&&conditionBbbbbbbbbbbb!==undefined&&{body:data})};

// Boundary: spread chain line 101 - breaks after the first `&&`; continuation indents one level
const brk={method:verb,...(conditionAaaaaaaaaaaaaaaaa!=='GET'&&conditionBbbbbbbbbbbb!==undefined&&{body:data})};
</script>
