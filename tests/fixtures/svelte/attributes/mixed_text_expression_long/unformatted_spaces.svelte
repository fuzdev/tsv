<!-- Short - no wrapping needed (under 100 chars) -->
<div title="prefix text {
a.prop || b.prop
} suffix"></div>

<!-- Long logical - wraps at || (exceeds 100 chars) -->
<div
	title="prefix text that is very long to reach print width {
	a.prop_name 
	||
		b.prop_name
		} suffix text"
></div>

<!-- Long ternary - wraps at ? and : (exceeds 100 chars) -->
<div
	title="prefix text reaching print width boundary with {
	cond

		?
		 a.prop_name_long

		: b.prop_name_long
		} more suffix"
></div>
