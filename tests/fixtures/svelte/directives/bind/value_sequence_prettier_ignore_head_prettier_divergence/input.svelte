<!-- a function-binding sequence freezes WHOLE under an own-line directive in the `{`→value
	gap, and stays bare: the getter/setter pair is not a grouped expression -->
<input
	bind:value={
		// prettier-ignore
		()  =>  a,
		(v)  =>  (a  =  v)
	}
/>
