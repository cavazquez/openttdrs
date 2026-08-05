/*
 * UI capture driver for openttdrs visual parity (#297, #299, #300, #301, #302).
 * Applied onto OpenTTD 15.3 at docs/parity/openttd-reference.json.
 */

#ifndef OPENTTDRS_UI_CAPTURE_H
#define OPENTTDRS_UI_CAPTURE_H

/**
 * Opens the window selected by OPENTTDRS_UI_CAPTURE_ID and queues a screenshot
 * named by OPENTTDRS_UI_CAPTURE_NAME. Both variables are intentionally inert
 * when absent, so the patched OpenTTD remains a normal interactive build.
 */
void OpenttdrsMaybeCaptureUi();

#endif /* OPENTTDRS_UI_CAPTURE_H */
