<script lang="ts">
	// a block comment glued to a stripped paren shell's closer trails the member
	enum E1 {A = (
1
)/* c */, B = 2}

	// a line comment in the same position defers past the comma
	enum E2 {A = (
1
)// c
, B = 2}

	// a block then a line comment each keep the side of the comma the author gave them
	enum E3 {A = (
1
)/* c1 */ // c2
, B = 2}

	// on the last member, whose comma is never emitted, the run trails it
	enum E4 {A = 1, B = (
2
)/* c */}

	// a line comment there defers past the member the same way
	enum E5 {A = 1, B = (
2
)// c
}

	// an own-line block comment with nothing before it on its line leads the next member
	enum E6 {A = 1
/* c */, B = 2}
</script>
