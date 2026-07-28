<script>
	// Line comment between ) and non-block body, else has comment
	if(a)// c1
	true;else// c2
	{expr;}

	// Block comment between ) and non-block body, else has comment
	if(a)/* c1 */true;else// c2
	{expr;}

	// else-if: line comment between ) and non-block body
	if(a){expr;}else if(b)// c1
	true;else// c2
	{expr;}

	// else-if: block comment between ) and non-block body
	if(a){expr;}else if(b)/* c1 */true;else// c2
	{expr;}

	// Both if and else-if have head-body comments
	if(a)// c1
	true;else if(b)// c2
	true;else// c3
	{expr;}

	// Block comment on else triggers same code path
	if(a)// c1
	true;else/* c2 */{expr;}

// Block comment glued to a keyword-opening body stays on the body's line
if(a)
// c1
/* c2 */ for (;;) {
			break;
		}
</script>
