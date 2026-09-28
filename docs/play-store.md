# Publishing NearClip on Google Play

Everything needed for the Play Console, in the order the console asks for it.
Copy the text blocks as they are. Policies change; when the console disagrees
with this file, the console is right — and please update this file.

## 1. Before anything else

| What | Cost | Notes |
| --- | --- | --- |
| Google Play developer account | 25 USD, one time | Personal or organisation. A personal account is enough for an individual developer; an organisation account needs a D-U-N-S number. |
| Identity verification | free | Address and ID documents. Takes a few days. |
| Closed test before production | free, but slow | Google requires personal accounts opened since late 2023 to run a closed test with a group of testers (12 at the time of writing) who stay opted in for 14 continuous days, before production access can be requested. Check the exact number in your console under **Test and release → Production → Apply for production access**. |

Plan for the closed test: it is the long pole, not the build. Recruit the
testers first, publish a closed-test release, then wait out the 14 days.

## 2. Build the bundle

Play takes an Android App Bundle (`.aab`), not an APK:

```sh
pnpm tauri android build --aab
# src-tauri/gen/android/app/build/outputs/bundle/universalRelease/app-universal-release.aab
```

The bundle is signed with the keystore described in the README. On Play that key
becomes the **upload key**: Google re-signs the app with its own key before it
reaches users.

Two consequences worth knowing:

- **Back up the keystore and its password.** Losing the upload key means asking
  Google to reset it; losing it without a backup is recoverable, losing the
  Play signing key is not (Google holds that one).
- **The Play build and the GitHub APK have different signatures.** Nobody can
  update from one to the other: they uninstall one and install the other. Say so
  in the README download section if both stay available.

`versionCode` comes from `src-tauri/gen/android/app/tauri.properties`, which
Tauri derives from the version in `tauri.conf.json`. Play refuses a bundle whose
`versionCode` is not higher than the last one uploaded, so every Play release
needs a version bump — the same bump that drives the GitHub release.

## 3. Store listing

**App name** (30 characters max)

```
NearClip
```

**Short description** (80 characters max)

```
Send clipboard text and files between your devices over your own Wi-Fi.
```

**Full description** (4000 characters max)

```
NearClip copies text and files straight from one of your devices to another over your local network. No cloud, no account, no servers in the middle.

Paste a link on your laptop and it is on your phone. Send a photo from your phone to your desktop without mailing it to yourself. Everything travels over the Wi-Fi you are already on.

WHAT IT DOES

• Send text, links, photos, videos and files up to 100 MB each
• Send to one paired device or to all of them at once
• Pair by scanning a QR code, or by typing an address when a network hides devices from each other
• See which of your devices are reachable right now
• Share to NearClip from any app through the Android share sheet
• Received files land in your Downloads folder; received text can go straight to your clipboard
• A history of what you sent and received, which you can clear whenever you like

PRIVATE BY DESIGN

Devices pair once by confirming the same six-digit code on both screens. After that, everything between them is encrypted end to end with a key only those two devices hold. Nothing is uploaded, because there is nowhere to upload it to: NearClip has no servers, and the developer cannot see anything you send.

NearClip asks for no location permission. That is why it tells you whether a device is reachable instead of naming the Wi-Fi network you are on.

WORKS WITH YOUR COMPUTER

NearClip also runs on macOS, Windows and Linux, and the desktop apps are free downloads on GitHub. Your phone and your computer pair the same way.

OPEN SOURCE

The whole app is open source under the MIT licence. Read the code, build it yourself, or send a patch: github.com/EmD-228/Nearclip

REQUIREMENTS

Both devices must be on the same local network — the same Wi-Fi, or a phone's hotspot with the other device connected to it. Some public and workplace networks stop devices from talking to each other; there NearClip cannot help.
```

**App category:** Tools
**Tags:** file sharing, clipboard, productivity
**Contact email:** denyok.emmanuel@gmail.com
**Website:** https://github.com/EmD-228/Nearclip
**Privacy policy URL:** https://github.com/EmD-228/Nearclip/blob/main/PRIVACY.md

## 4. Graphics

| Asset | Requirement | Where |
| --- | --- | --- |
| App icon | 512 × 512 PNG, no transparency | `docs/play/icon-512.png` |
| Feature graphic | 1024 × 500 PNG | `docs/play/feature-graphic.png` |
| Phone screenshots | 2 to 8, 16:9 or 9:16, 1080 × 1920 works | take on the phone |
| Tablet screenshots | optional, but without them the listing is marked as not designed for tablets | optional |

Screenshots to take on the phone, in this order — the first two are what most
people see:

1. **Devices** with a paired computer showing `Connected`, and its recent
   exchanges underneath.
2. **Send** with a photo attached and the progress bar running.
3. **History** with a received file and a received text.
4. The **pairing code** dialog, or the QR scanner.

## 5. Data safety form

NearClip collects nothing, which makes this form short. Answers:

- **Does your app collect or share any of the required user data types?** No.
- **Is all of the user data collected by your app encrypted in transit?** Not
  applicable — but if the form still asks, the answer is yes: everything between
  devices is encrypted end to end.
- **Do you provide a way for users to request that their data is deleted?** Not
  applicable; nothing is collected. The History screen clears local history.

If the form pushes back because the app reads photos and files: those are
selected by the user and sent only to the user's own devices; they are neither
collected nor shared, in the form's sense.

## 6. Content rating questionnaire

Category: **Utility, productivity, communication or other**. Every content
question is No — no violence, no sexual content, no profanity, no gambling, no
drugs, no user-generated content shared with strangers, no location sharing, no
personal information collection, no digital purchases. Expected result: rated
for everyone (PEGI 3 / ESRB Everyone).

## 7. Declarations

- **Ads:** No, the app contains no ads.
- **Target audience:** 13 and over. The app has no content for children and no
  reason to enter the Families programme.
- **Foreground service:** the app declares
  `FOREGROUND_SERVICE_CONNECTED_DEVICE`, so the console asks for a short
  justification and a video. Justification:

  ```
  NearClip keeps a short-lived foreground service while it receives a file from another device the user has paired with, so a transfer in progress is not killed when the screen turns off. The service stops as soon as the transfer ends. No data leaves the local network.
  ```

- **Government app, financial features, health:** No to all.
- **Data deletion URL:** not needed; nothing is collected.

## 8. Release notes

Keep them to what a user notices. For the first release:

```
First release on Google Play.

• Send clipboard text, photos, videos and files to your own devices over your local network
• Pair with a QR code or by address, with a six-digit code confirmed on both screens
• End-to-end encryption between paired devices, no account and no servers
• Also available for macOS, Windows and Linux at github.com/EmD-228/Nearclip
```

## 9. Publishing from CI

Once the first bundle has been uploaded by hand — Google requires that, the API
cannot create an app — a `v*` tag can do the rest. The `play` job in
`.github/workflows/build.yml` builds the bundle and sends it to a track.

**What to set up, once:**

1. **Play Console → Setup → API access**. Link a Google Cloud project, then
   create a service account. The console walks through it.
2. In Google Cloud, give that service account a key: **Keys → Add key → JSON**.
   Download it. It is a password to your listing; it is never committed.
3. Back in **Play Console → Users and permissions**, invite the service account
   address and grant it, for this app only: **Release apps to testing tracks**
   and **Release to production** if you want CI to reach production. Nothing
   else.
4. In GitHub, **Settings → Secrets and variables → Actions → New repository
   secret**, name `PLAY_SERVICE_ACCOUNT_JSON`, paste the whole JSON file.

Without that secret the job builds the bundle and stops, so a fork never tries
to publish.

**What happens on a tag:** the bundle goes to the `alpha` track, released to its
testers. That is the API's name for the closed test the console shows as "Tests
fermés - Alpha", and it is the track whose testers have to stay opted in for the
two weeks Google counts before production opens up. To aim elsewhere, run the
workflow by hand from the Actions tab and set `play_track` to `internal`, `beta`
or `production` — or to the name of a custom track, as the console spells it.

**Release notes** come from `distribution/whatsnew/whatsnew-<locale>`, one short
file per listing language. They are part of the commit, so rewrite them with
each version; they are what testers read in Play, not the GitHub changelog.

**`versionCode`** is derived from the version in `tauri.conf.json`, and Play
refuses a bundle whose code is not higher than the last. So a Play release needs
a version bump, the same one that drives the GitHub release.

## 10. After the first release

- Bump the version in `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` and
  `package.json`, so `versionCode` grows.
- Upload the new `.aab` to the same track, with release notes.
- Keep the GitHub release going for the desktop builds and for people who prefer
  the APK.
- Automation is possible later: a service account plus a GitHub Action can push
  the bundle to a Play track, and `.github/workflows/build.yml` already builds
  and signs Android.
