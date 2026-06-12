<style>
	input:checked {
		background: green;
	}

	button:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	input:enabled {
		border-color: blue;
	}

	input:focus {
		outline: 2px solid blue;
	}

	a:hover {
		text-decoration: underline;
	}

	a:active {
		color: red;
	}

	input:required {
		border-left: 3px solid red;
	}

	input:optional {
		border-left: 3px solid gray;
	}

	input:user-valid {
		border-color: green;
	}

	input:user-invalid {
		border-color: red;
	}

	input:autofill {
		background: lightyellow;
	}

	input:valid {
		border-color: green;
	}

	input:invalid {
		border-color: red;
	}

	input:in-range {
		background: lightgreen;
	}

	input:out-of-range {
		background: lightcoral;
	}

	input:read-only {
		background: lightgray;
	}

	input:read-write {
		background: white;
	}

	input:placeholder-shown {
		color: gray;
	}

	button:default {
		font-weight: bold;
	}

	input:indeterminate {
		opacity: 0.5;
	}

	button:focus-visible {
		outline: 2px solid blue;
	}
</style>
