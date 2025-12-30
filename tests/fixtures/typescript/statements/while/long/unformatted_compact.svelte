<script lang="ts">
// With braces

// 100 chars exactly - stays inline (at boundary)
while(condA&&condB&&condC&&condD&&condE&&condF&&condG&&condH&&condIIIIIIIIIII){}

// 101 chars - wraps (exceeds print_width)
while(condA&&condB&&condC&&condD&&condE&&condF&&condG&&condH&&condIIIIIIIIIIII){}

// No braces, body on same line (entire statement fits)

// 100 chars exactly - entire statement stays on one line
while(condA&&condB&&condC&&condD&&condE&&condF&&condG&&condH&&condIIIIIIIIIII)a;

// No braces, body on next line (condition too long for body to fit)

// 100 chars for condition only - body goes to next line
while(condA&&condB&&condC&&condD&&condE&&condF&&condG&&condH&&condI&&condJJJJJ)a;

// 101 chars - condition wraps
while(condA&&condB&&condC&&condD&&condE&&condF&&condG&&condH&&condI&&condJJJJJJ)a;
</script>
