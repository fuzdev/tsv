<!-- 100 chars -->
<div>
	<small>{a !== 1 ? 's' : ''}{b !== 1 ? 's' : ''}{ccccccccccccccccccccccccccccccccccccccccc}</small>
</div>

<!-- 101 chars: <tag>content</tag> exceeds but <tag>content</tag fits, only closing > breaks -->
<div>
	<small>
		{a !== 1 ? 's' : ''}{b !== 1 ? 's' : ''}{cccccccccccccccccccccccccccccccccccccccccc}
	</small>
</div>

<!-- 102 chars: we break second ternary; Prettier breaks opening bracket -->
<div>
	<small>
		{a !== 1 ? 's' : ''}{b !== 1 ? 's' : ''}{ccccccccccccccccccccccccccccccccccccccccccc}
	</small>
</div>

<!-- Logical chain: we break at &&; Prettier breaks opening bracket -->
<div>
	<small>
		{aaa && bbb && ccc && ddd && eee && fff && ggg && hhh && iii && jjj && kkk && lll && mmm}
	</small>
</div>

<!-- Binary chain: we break at +; Prettier breaks opening bracket -->
<div>
	<small>
		{arr[0] + arr[1] + arr[2] + arr[3] + arr[4] + arr[5] + arr[6] + arr[7] + arr[8] + arr[9]}
	</small>
</div>
