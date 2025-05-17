prev = Vorige
next = Volgende
unknown-os = Onbekend besturingssysteem
parttype-root = Tophiërarchie bestandssysteem (/) ({ $path })
parttype-extendedboot = Extended Bootloader-partitie ({ $path })
parttype-esp = EFI-systeempartitie ({ $path })
parttype-home = Gebruikerspartitie ({ $path })
parttype-var = Partitie voor variabele gegevens (/var) ({ $path })
parttype-other = Mountpoint aangepaste partitionering
page-welcome = Welkom bij { $distro }
page-welcome-desc = U kunt { $distro } proberen of de installatie nu starten.
page-welcome-try = Eerst uitproberen
page-welcome-install = Installeren
page-failure = Installatie mislukt
page-failure-close = Sluiten
page-failure-bug = Een bug melden
page-language = Taal
page-language-search-lang = Naar taal en regio zoeken…
page-language-next = Volgende
page-completed = Voltooid
page-completed-desc = Installatie voltooid. Start de computer opnieuw op om te genieten van uw nieuwe systeem.
page-completed-close = Sluiten
page-completed-reboot = Opnieuw opstarten
page-destination = Einddoel
page-destination-scanning = Schijven scannen
page-destination-wait = Wachten op os-prober…
page-destination-no-disk = Geen schijven gevonden
page-destination-no-disk-desc = Er zijn geen schijven die geschikt zijn voor installatie.
page-installdual = Dualboot
page-installdual-otheros = Ander besturingssysteem
page-confirmation = Bevestiging
page-confirmation-problem-device-mounted = { $dev } is aangekoppeld op { $mountpoint }. Ontkoppel om door te gaan.
page-confirmation-problem-devblkopen =
    Het blok-device <tt>{ $dev }</tt> wordt gebruikt door de volgende processen:
    <tt>{ $pids }</tt>
    Deze processen moeten worden afgesloten voordat de installatie verder kan gaan.
page-installation = Installatie
page-installation-welcome-desc = Leer uw nieuwe besturingssysteem kennen.
page-installation-help = Hulp nodig?
page-installation-help-desc = Vraag het in een van onze chats!
page-installation-contrib = Draag bij aan { $distro }
page-installation-contrib-desc = Ontdek hoe u tijd, geld of hardware kunt bijdragen.
page-installation-progress = Basissysteem installeren...
page-installcustom = Aangepaste installatie
page-installcustom-title = Partities en mountpoints
page-installcustom-desc =
    { $num } { $num ->
        [one] definitie
       *[other] definities
    }
page-installcustom-tool = Partitioneringstool openen
page-installcustom-add = Een nieuwe definitie/rij toevoegen
page-installationtype = Type installatie
page-installationtype-entire = Gehele schijf
page-installationtype-tpm = TPM inschakelen
page-installationtype-encrypt = Schijfversleuteling inschakelen
page-installationtype-chromebook = Chromebook
page-installationtype-dual = Dualboot
page-installationtype-custom = Aangepast
dialog-installtype-encrypt = Schijfversleuteling
dialog-installtype-encrypt-desc =
    Stel het wachtwoord voor schijfversleuteling in.
    Als u het wachtwoord verliest, kunnen uw gegevens niet worden hersteld.
dialog-installtype-password = Wachtwoord
dialog-installtype-repeat = Wachtwoord herhalen
dialog-installtype-cancel = Annuleren
dialog-installtype-confirm = Bevestigen
installtype-edit-mp = Mountpoint bewerken
installtype-rm-mp = Mountpoint verwijderen
dialog-mp-part = Partitie
dialog-mp-at = Mountpoint
dialog-mp-opts = Mount-opties
installtype-parttool = Selecteer uw partitioneringstool
stage-extracting = Bestanden uitpakken
stage-copying = Bestanden kopiëren
stage-mkpart = Partities aanmaken en bestanden kopiëren
stage-initramfs = Initiramfs opnieuw genereren
stage-grub = Standaardinstellingen systeem grub genereren
stage-grub1 = Stage 1 grub.cfg genereren in ESP...
stage-grub2 = Stage 2 grub.cfg genereren in /boot/grub2/grub.cfg...
stage-biosgrub = BIOS Grub2 installeren
stage-kernel = Kernels opnieuw installeren
stage-selinux = SELinux labels instellen
