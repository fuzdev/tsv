<style>
	/* Basic scoping */@scope (.class1) {img {border: 1px solid;}}

	/* Scope with limit */@scope (.class1) to (.class2) {p {color: blue;}}

	/* Multiple selectors */@scope (.class1, .class3) {.class4 {font-weight: bold;}}

	/* Complex limit */@scope (.class5) to (.class6, .class7) {a {text-decoration: underline;}}

	/* Nested scopes */@scope (.class8) {.class9 {padding: 1rem;}

@scope (.class10) {span {font-weight: bold;}}}
</style>
