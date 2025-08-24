# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at http://mozilla.org/MPL/2.0/.

prev = Wróć
next = Idź
page-welcome-try = Wypróbuj
page-welcome-install = Zainstaluj
page-failure = Typ instalacji
page-language-next = Idź
page-destination = Wolumen docelowy
page-confirmation = Potwierdzenie
page-installation = Instalacja
page-installation-progress = Instalowanie podstawowych komponentów systemu...
page-installationtype = Typ instalacji
unknown-os = Nieznany system
parttype-root = Katalog główny systemu plików ({ $path })
parttype-extendedboot = Rozszerzona partycja programu rozruchowego ({ $path })
parttype-esp = Partycja systemowa EFI ({ $path })
parttype-home = Dane użytkownika ({ $path })
parttype-var = Dane zmiennych ({ $path })
parttype-other = Niestandardowy punkt montowania partycji
page-welcome = Witaj w { $distro }
page-welcome-desc = Możesz wypróbować { $distro } lub rozpocząć instalację.
page-failure-close = Zamknij
page-failure-bug = Zgłoś błąd
page-language = Język
page-language-search-lang = Wyszukaj języka/localu…
page-completed = Zakończ
page-completed-desc = Zakończono instalację. Możesz teraz uruchomić komputer ponownie i cieszyć się świeżutkim systemem.
page-completed-close = Zamknij
page-completed-reboot = Uruchom ponownie
page-destination-scanning = Skanowanie dysków
page-destination-wait = Oczekiwanie na os-prober…
page-destination-no-disk = Nie znaleziono żadnych dysków
page-destination-no-disk-desc = Nie ma dysków odpowiednich do instalacji.
page-installdual = Dual Boot
page-installdual-otheros = Inny system
page-confirmation-problem-device-mounted = { $dev } jest zamontowany na { $mountpoint }. Odmontuj, aby kontynuować.
page-confirmation-problem-devblkopen =
    Block-device <tt>{ $dev }</tt> jest używany przez następujące procesy:
    <tt>{ $pids }</tt>
    Należy zakończyć te procesy, aby instalacja mogła być rozpoczęta.
page-installation-welcome-desc = Poznaj swój nowy system operacyjny.
page-installation-help = Potrzebujesz pomocy?
page-installation-help-desc = Poproś o nią w jednym z naszych czatów!
page-installation-contrib = Miej wkład w { $distro }
page-installation-contrib-desc = Dowiedz się jak wnosić swój czas, pieniądze lub sprzęt.
page-installcustom = Instalacja niestandardowa
page-installcustom-title = Partycje i punkty montowania
page-installcustom-desc =
    { $num } { $num ->
        [jedna] definicja
       *[inne] definicje
    }
page-installcustom-tool = Otwórz narzędzie do partycjonowania
page-installcustom-add = Dodaj nową definicję/wiersz
page-installationtype-entire = Cały dysk
page-installationtype-tpm = Włącz TPM
page-installationtype-encrypt = Włącz szyfrowanie dysku
page-installationtype-chromebook = Chromebook
page-installationtype-dual = Dual Boot
page-installationtype-custom = Niestandardowe
dialog-installtype-encrypt = Szyfrowanie dysku
dialog-installtype-encrypt-desc =
    Proszę ustawić hasło szyfrowania dysku.
    Jeśli stracisz hasło, twoje dane nie będą odzyskiwalne.
dialog-installtype-password = Hasło
dialog-installtype-repeat = Powtórz hasło
dialog-installtype-cancel = Anuluj
dialog-installtype-confirm = Potwierdź
installtype-edit-mp = Edytuj punkt montowania
installtype-rm-mp = Usuń punkt montowania
dialog-mp-part = Partycja
dialog-mp-at = Zamontuj w
dialog-mp-opts = Opcje montowania
installtype-parttool = Wybierz swoje narzędzie do partycjonowania
stage-extracting = Wypakowywanie plików
stage-copying = Kopiowanie plików
stage-mkpart = Tworzenie partycji i kopiowanie plików
stage-initramfs = Ponowne generowanie initramfs
stage-grub = Generowanie domyślnych wartości dla GRUB
stage-grub1 = Generowanie etap 1 grub.cfg w ESP...
stage-grub2 = Generowanie etap 2 grub.cfg w /boot/grub2/grub.cfg...
stage-biosgrub = Instalowanie BIOSowego Grub2
stage-kernel = Ponowna instalacja jąder
stage-selinux = Ustawianie etykiet SELinux
err-no-bios = Nie wykryto /sys/firmware/efi, a dystrybucja wyłączyła wsparcie BIOSu.
