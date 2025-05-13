# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at http://mozilla.org/MPL/2.0/.

prev = Edellinen
next = Seuraava
page-welcome-try = Yritä
page-welcome-install = Asemma
page-failure = Asennus virhe
page-failure-close = Lähellå
page-failure-bug = Ilmoita virhe
page-language = Kieli
page-language-next = Seruraava
page-completed-desc = Asennus on valmis. Voit käynistää uudelleen ja nautia sinun uudesta järjestemästä.
page-completed-close = Lähellå
page-completed-reboot = Käynistä uudelleen
page-destination = Kohde
page-confirmation = Varmistus
page-installation = Asennus
page-installation-progress = Asentamassa kantajärjestelmää...
page-installationtype = Asennuksen Tyyppi
page-installationtype-chromebook = Chromebook
unknown-os = Tuntematon käyttöjärjestelmä
page-destination-no-disk-desc = Ei asennukseen sopivia levyjä.
page-installationtype-tpm = Ota TPM käyttöön
dialog-installtype-confirm = Hyväksy
installtype-rm-mp = Poista liitospiste
parttype-other = Muiden osioiden liitospisteet
dialog-mp-at = Liitä pisteeseen
stage-copying = Kopioidaan tiedostoja
page-welcome-desc = Voit kokeilla järjestelmää { $distro } tai asentaa sen nyt.
dialog-mp-opts = Liitosvaihtoehdot
parttype-extendedboot = Laajennettu käynnistyslataimen osio (XBOOTLDR) { $path })
page-language-search-lang = Etsi kieltä/lokalisointia…
stage-grub1 = Luodaan vaiheen 1 käynnistyslataajaa ESP:lle...
parttype-root = Juurihakemisto ({ $path })
parttype-esp = EFI järjestelmäosio ({ $path })
parttype-home = Käyttäjien data ({ $path })
parttype-var = Muuttuja data ({ $path })
page-welcome = Tervetuloa järjestelmään { $distro }
page-completed = Valmis
page-destination-scanning = Etsitään levyjä
page-destination-wait = Etsitään käyttöjärjestelmiä…
page-destination-no-disk = Ei löydettyjä levyjä
page-installdual = Dual Boot
page-installdual-otheros = Muu käyttöjärjestelmä
page-confirmation-problem-device-mounted = { $dev } on liitetty hakemistoon { $mountpoint }. Irroita se jatkaaksesi.
page-confirmation-problem-devblkopen =
    Levyä <tt>{ $dev }</tt> käyttää seuraavat prosessit:
    <tt>{ $pids }</tt>
    Nämä prosessit on lopetettava jatkaaksesi asennusta.
page-installation-welcome-desc = Tutustu uuteen käyttöjärjestelmääsi.
page-installation-help = Tarvitsetko apua?
page-installation-help-desc = Kysy chatissä!
page-installation-contrib = Osallistu { $distro }n kehitykseen
page-installation-contrib-desc = Opi kuinka voit auttaa ajallasi, rahallisesti tai laitteistollasi.
page-installcustom = Mukautettu asennus
page-installcustom-title = Osiot ja liitospisteet
page-installcustom-desc = { $num } määritelmä(t)
page-installcustom-tool = Avaa osiointityökalu
page-installcustom-add = Lisää uusi määrittely/rivi
page-installationtype-entire = Koko levy
page-installationtype-encrypt = Ota levyn salaus käyttöön
page-installationtype-dual = Dual Boot
page-installationtype-custom = Mukautettu
dialog-installtype-encrypt = Levyn salaus
dialog-installtype-encrypt-desc =
    Aseta levyn salauksen salasana.
    Jos unohdat salasanan, tietojasi ei voida palauttaa.
dialog-installtype-password = Salasana
dialog-installtype-repeat = Anna salasana uudelleen
dialog-installtype-cancel = Peruuta
installtype-edit-mp = Muokkaa liitospistettä
dialog-mp-part = Osio
installtype-parttool = Valitse osiointityökalu
stage-extracting = Puretaan tiedostoja
stage-mkpart = Luodaan osioita ja kopioidaan tiedostoja
stage-initramfs = Uudistetaan initramfs
stage-grub = Luodaan GRUB asetuksia
stage-grub2 = Luodaan vaiheen 2 käynnistyslataajaa kohteeseen /boot/grub2/grub.cfg...
stage-biosgrub = Asennetaan BIOS Grub2
stage-kernel = Asennetaan ytimiä uudelleen
stage-selinux = Asetetaan SELinux nimiöitä
