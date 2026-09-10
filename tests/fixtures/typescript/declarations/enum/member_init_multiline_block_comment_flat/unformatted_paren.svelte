<script lang="ts">
	// A multi-line block comment leading an enum member's initializer must not BREAK the
	// value's own group: prettier prints a leading comment outside that group, so a value
	// that fits stays flat below the comment. The member keeps `= value` inline (it never
	// hangs under the `=` — see member_init_multiline_block_comment), so this seam reads
	// the comment for itself.
	enum E {
		// binary chain
		A = /**
		 * c
		 */ (b + c + d),

		// ternary
		B = /**
		 * c
		 */ (e ? f : g),

		// member chain
		C = /**
		 * c
		 */ h.i().j().k(),

		// a preserved (non-indentable) multi-line block asks the same question
		D = /* c
	e */ (l + m + n),

		// null control: a single-line block forces no break
		F = /* c */ o + p + q,

		// null control: no comment
		G = r + s + t
	}
</script>
