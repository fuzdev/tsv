<script lang="ts">
	// same-line type arguments bind to the preceding type: one member `a: B<T>`
	type Same = { a: B<T> }

	// a same-line block comment is trivia, not a line break, so the type
	// arguments still bind to the preceding type
	type CommentBind = { a: B/* c */ <T> }

	// a qualified head binds its type arguments the same way
	type Qual = { a: P.Q<T> }

	// a line break ends the `a: B` member; `<T>(): C` is a distinct
	// call-signature member (two members)
	type Split = {
		a: B
		<T>(): C
	}

	interface Split2 {
		a: B
		<T>(): C
	}
</script>
