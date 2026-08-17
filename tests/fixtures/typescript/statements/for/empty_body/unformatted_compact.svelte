<script>
// regular for with empty body (already collapses)
for(;;);

// for-in with empty body
for(a in b);

// for-of with empty body
for(a of b);

// A comment run in the )-to-; gap stays trailing ) with the ; flush beneath it,
// since an empty body is not an adjustClause clause and takes no indent
for(;;)/* c1 */// c2
;

for(a in b)/* c1 */// c2
;

for(a of b)/* c1 */// c2
;

// A block comment alone keeps the ; on the header's line
for(a of b)/* c1 */;
</script>
