<!-- "Agent working" indicator: a solid dot in the project colour with a pulsing ring around it.
     Expects --project to be set by an ancestor.
     The ring only animates opacity: a growing ring (animate-ping) forces WebKitGTK to repaint
     every frame, and behind a dialog's backdrop that made the whole UI lag. -->
<span class="relative flex size-2.5 shrink-0" title="Agent working">
  <span class="absolute -inset-1 rounded-full bg-(--project)/40 motion-safe:animate-pulse"></span>
  <span class="relative inline-flex size-2.5 rounded-full bg-(--project)"></span>
</span>
