<script>
function f1(){
// Return: trailing block comment inline
return a+b/* c */;
}

function f2(){
// Return: && trailing
return a&&b/* c */;
}

function f3(){
// Return: combined leading + trailing
return/* l */a+b/* t */;
}

// Inside function call argument
fn(a+b/* c */);
fn(a&&b/* c */);

// Statement-level (block comment trails after semicolon)
a+b/* c */;
a&&b/* c */;
</script>
