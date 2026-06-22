<!-- td at exactly 100 chars - fits on one line -->
<td><a href="x">xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx</a></td>

<!-- td at 101 chars - block-style: td intact, <a> on its own indented line -->
<td>
	<a href="x">xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx</a>
</td>

<!-- td at 107 chars - same block-style shape; the inner <a> content still fits on its line -->
<td>
	<a href="x">xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx</a>
</td>

<!-- td at 109 chars - block-style cascades: the inner <a> content also moves to its own line -->
<td>
	<a href="x">
		xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
	</a>
</td>

<!-- th at 101 chars - same block-style behavior -->
<th>
	<a href="x">xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx</a>
</th>

<!-- tr at 101 chars - td inside tr lays out block-style -->
<tr>
	<td>
		<a>xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx</a>
	</td>
</tr>

<!-- tbody at 101 chars - nested tr/td lay out block-style -->
<tbody>
	<tr>
		<td><a>xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx</a></td>
	</tr>
</tbody>

<!-- thead at 101 chars -->
<thead>
	<tr>
		<th><a>xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx</a></th>
	</tr>
</thead>

<!-- tfoot at 101 chars -->
<tfoot>
	<tr>
		<td><a>xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx</a></td>
	</tr>
</tfoot>
