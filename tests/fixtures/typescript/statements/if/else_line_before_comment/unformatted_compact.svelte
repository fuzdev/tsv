<script>
// Line comment before else - no blank line above
if(cond1){fn1();}
// comment (no blank above)
else if(cond2){fn2();}

// Line comment before else - blank line above (preserved)
if(cond1){fn1();}

// comment (blank above)
else{fn2();}

// Trailing comment before else - stays attached (existing behavior)
if(cond1){fn1();} // trailing comment
else{fn2();}
</script>
