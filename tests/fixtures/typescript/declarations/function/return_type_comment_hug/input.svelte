<script lang="ts">
	// A single parameter stays flat (hugs) when a multi-line block comment in the
	// `)`→`:` gap breaks the signature: the comment and return type break while the
	// param keeps its line, matching prettier. Covers all four signature builders.

	// class method
	class C {
		m(a: string) /* aa
		bb */ : string {}
	}

	// function declaration
	function f(a: string) /* aa
	bb */ : string {}

	// object method
	const o = {
		m(a: string) /* aa
		bb */ : string {}
	};

	// function expression
	const g = function (a: string) /* aa
	bb */ : string {};
</script>
