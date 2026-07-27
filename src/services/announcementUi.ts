/**
 * Tiny shared handle for opening the announcement history dialog.
 *
 * The dialog lives inside `AnnouncementModal` (mounted once at the app
 * root, so it can overlay any route), but the affordances that open it
 * are elsewhere — the title-bar banner and the Settings page. Rather
 * than thread refs through the router or a store, those call
 * {@link openAnnouncementList} and the modal reacts.
 *
 * Kept dependency-free (like `windowFit`) so a component can import the
 * opener without pulling the modal's Tauri imports into a unit test.
 */

import { ref, type Ref } from 'vue'

const listOpen = ref(false)

/** Reactive "the history dialog is open" flag, owned by the modal. */
export function announcementListOpen(): Ref<boolean> {
  return listOpen
}

/** Open the announcement history dialog from anywhere in the app. */
export function openAnnouncementList(): void {
  listOpen.value = true
}

/** Close it (the dialog's own controls). */
export function closeAnnouncementList(): void {
  listOpen.value = false
}
