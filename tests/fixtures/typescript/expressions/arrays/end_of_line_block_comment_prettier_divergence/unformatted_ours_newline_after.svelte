<script>
	// Block comment after the comma leads the next element - tsv keeps it there
	const a = ['aaaa', /* c */
'bbbb', 'cccc'];

	// Same with a following hole - the hole must not re-collect the comment
	const b = ['aaaa', /* c */
'bbbb', , 'cccc'];

	// Two holes
	const c = ['aaaa', /* c */
'bbbb', , 'cccc', , 'dddd'];

	// Contrast - a comment before the comma trails the previous element
	const d = ['aaaa' /* c */,
'bbbb', 'cccc'];

	// A line comment expands the array - the glued comment still leads its element
	const e = ['aaaa', /* c1 */ 'bbbb', // c2
'cccc'];

	// Expanded with a hole
	const f = ['aaaa', /* c1 */ 'bbbb', // c2
, 'cccc'];

	// A newline after the comment unglues it - the comment takes its own line
	const g = ['aaaa', /* c1 */
'bbbb', // c2
'cccc'];
</script>
