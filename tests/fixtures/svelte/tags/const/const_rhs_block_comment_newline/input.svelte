{#if cond}
	<!-- A block glued to the `=`, the value on the next line: the newline collapses when it fits -->
	{@const a = /* c */ b}
	{@const c = /* c */ { d: 1 }}
	{@const e = /* c */ (f) => (g) => h}
	{@const i = /* c1 */ /* c2 */ j}

	<!-- A value too long for the line breaks after the `=`, the comment on its own line above it -->
	{@const k =
		/* c */
		fn(argumentNumberOne, argumentNumberTwo, argumentNumberThree, argumentNumberFour)}

	<!-- An own-line comment (a newline precedes it) keeps the break after `=` -->
	{@const l =
		/* c */
		m}

	<p>{a}{c}{e}{i}{k}{l}</p>
{/if}
