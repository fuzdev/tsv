<!-- Block child at indent 1 - triggers the bug (101 effective) -->
<div><button type="button" class="class1 class2 class3 class4 class5 class6 class7 class8 class9" onclick={() => fn(arg)}> <div>content</div>	</button></div>

<!-- Text child at indent 1 - works correctly (same attrs) -->
<div> <button type="button" class="class1 class2 class3 class4 class5 class6 class7 class8 class9" onclick={() => fn(arg)}> content </button></div>

<!-- Block child at root - works correctly -->
<button type="button" class="class1 class2 class3 class4 class5 class6 class7 class8 class9" onclick={() => fn(arg)}> <div> content </div> </button>
