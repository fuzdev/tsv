<script lang="ts"></script>

<!-- An expression tag as the content of an inline element that trailing text follows after a space, at the 100/101 boundary of the line where the expression breaks: a curried arrow chain's heads, an assignment's break after =, and a type parameter's break after extends or =. At 101 each breaks and indents what follows it exactly as it does without the trailing text. -->

<!-- heads line == 100: the curried chain's heads stay on one line -->
<div>
	<b>
		{(axxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx) => (bbbb) =>
			cccc}
	</b> tail
</div>

<!-- heads line == 101: the heads break and the body indents under the last one -->
<div>
	<b>
		{(axxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx) =>
			(bbbb) =>
				cccc}
	</b> tail
</div>

<!-- assignment line == 100: the value stays after the = -->
<div>
	<b>
		{(axxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx = cyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy
			? byyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy
			: dyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy)}
	</b> tail
</div>

<!-- assignment line == 101: the value breaks after the =, its own lines indented -->
<div>
	<b>
		{(axxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx =
			cyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy
				? byyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy
				: dyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy)}
	</b> tail
</div>

<!-- constraint line == 100: the constraint stays after extends -->
<div>
	<b>
		{<
			Txxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx extends Foo<
				Ayyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy,
				Byyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy
			>
		>(
			a: T
		) => a}
	</b> tail
</div>

<!-- constraint line == 101: the constraint breaks after extends, its own lines indented -->
<div>
	<b>
		{<
			Txxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx extends
				Foo<
					Ayyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy,
					Byyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy
				>
		>(
			a: T
		) => a}
	</b> tail
</div>

<!-- default line == 100: the default stays after the = -->
<div>
	<b>
		{<
			U,
			Txxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx = Foo<
				Ayyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy,
				Byyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy
			>
		>(
			a: T
		) => a}
	</b> tail
</div>

<!-- default line == 101: the default breaks after the =, its own lines indented -->
<div>
	<b>
		{<
			U,
			Txxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx =
				Foo<
					Ayyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy,
					Byyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy
				>
		>(
			a: T
		) => a}
	</b> tail
</div>
