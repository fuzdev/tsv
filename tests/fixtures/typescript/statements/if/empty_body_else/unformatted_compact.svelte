<script>
// Empty consequent, empty alternate
if(a);else;

// Empty consequent, block alternate
if(a);else{expr;}

// Empty consequent, expression alternate
if(a);else expr;

// Chained: all empty
if(a);else if(b);else;

// Chained: empty consequent, blocks after
if(a);else if(b){expr;}else{expr;}
</script>
