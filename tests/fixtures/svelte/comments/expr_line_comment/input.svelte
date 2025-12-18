<!-- Line comment with apostrophe in expression -->
<div
	onclick={() => {
		// item's value
		fn();
	}}
></div>

<!-- Line comment with double quote in expression -->
<div
	onclick={() => {
		// the "quoted" value
		fn();
	}}
></div>

<!-- Line comment without quotes (should work) -->
<div
	onclick={() => {
		// plain comment
		fn();
	}}
></div>
