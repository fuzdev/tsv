<script>
let a;

// block comment before identifier
a=/* comment */expr;

// block comment before function expression
a=/** @param {number} x */function(x){};

// compound assignment with block comment
a+=/* comment */expr;

// block comment before object literal
a=/* comment */{b:1};

// block comment before array literal
a=/* comment */[1,2];

// block comment before call expression
a=/* comment */fn();
</script>
