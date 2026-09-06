<script>
// An empty object value: `= {};` lands on column 100 and stays on the line
const aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa={};

// One column more: the value drops after the `=` (101 visual)
const bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb={};

// An empty array, function and arrow drop the same way (101 visual)
const ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc=[];const ddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd=function(){};const eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee=()=>{};

// A class expression never breaks after the `=`: it stays on the line (101 visual)
const fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff=class{};
</script>
