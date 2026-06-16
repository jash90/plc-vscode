(*═══════════════════════════════════════════════════════════════════════════════
  PLIK TESTOWY: Structured Text (ST) — IEC 61131-3
  ───────────────────────────────────────────────────────────────────────────────
  Cel:  Sprawdzić, czy kod kompiluje się i liczy poprawnie. Każdy fragment
        wykonuje obliczenie i ZAPISUJE wynik do zmiennej tekstowej sLogXX
        w formacie 'opis = wynik'. Wyniki PODGLĄDASZ ONLINE (Watch / Monitoring).

  JAK URUCHOMIĆ (CODESYS / TwinCAT):
    1. Utwórz nowy PROGRAM (POU typu PROGRAM) i wklej całą zawartość.
    2. Dodaj wywołanie PRG_Test w zadaniu (Task) — np. cyklicznym 100 ms.
    3. Zaloguj się do sterownika/symulatora (Login) i uruchom (Run).
    4. Otwórz POU online lub dodaj zmienne sLog01..sLogXX do okna Watch.
       Obok każdej zmiennej zobaczysz tekst typu:  "2 + 3 = 5".
    5. Porównaj z komentarzem  // oczekiwane: ...  przy każdej linii.

  UWAGA: czysty ST nie ma funkcji "print". Tutaj "wypisywaniem" jest zapis do
  zmiennej STRING, którą widać na żywo w trybie online. To standardowy sposób
  obserwacji wyników na PLC. (Opcjonalne logowanie do konsoli — patrz koniec pliku.)
═══════════════════════════════════════════════════════════════════════════════*)

PROGRAM PRG_Test
VAR
    // ─────────────────────────────────────────────────────────────────────────
    //  ZMIENNE "WYJŚCIOWE" — to je obserwujesz online (Watch).
    //  Każda zawiera opis + obliczony wynik.
    // ─────────────────────────────────────────────────────────────────────────
    sLog01 : STRING[80];   sLog02 : STRING[80];   sLog03 : STRING[80];
    sLog04 : STRING[80];   sLog05 : STRING[80];   sLog06 : STRING[80];
    sLog07 : STRING[80];   sLog08 : STRING[80];   sLog09 : STRING[80];
    sLog10 : STRING[80];   sLog11 : STRING[80];   sLog12 : STRING[80];
    sLog13 : STRING[80];   sLog14 : STRING[80];   sLog15 : STRING[80];
    sLog16 : STRING[80];   sLog17 : STRING[80];   sLog18 : STRING[80];
    sLog19 : STRING[80];   sLog20 : STRING[80];   sLog21 : STRING[80];
    sLog22 : STRING[80];   sLog23 : STRING[80];   sLog24 : STRING[80];
    sLog25 : STRING[80];   sLog26 : STRING[80];   sLog27 : STRING[80];
    sLog28 : STRING[80];   sLog29 : STRING[80];   sLog30 : STRING[80];

    // ─────────────────────────────────────────────────────────────────────────
    //  ZMIENNE ROBOCZE
    // ─────────────────────────────────────────────────────────────────────────
    // arytmetyka
    iA      : INT  := 2;
    iB      : INT  := 3;
    iWynik  : INT;
    diWynik : DINT;
    rA      : REAL := 7.0;
    rB      : REAL := 2.0;
    rWynik  : REAL;

    // logika / bity
    xA      : BOOL := TRUE;
    xB      : BOOL := FALSE;
    xWynik  : BOOL;
    wA      : WORD := 16#000F;   // = 15
    wB      : WORD := 16#00F0;   // = 240
    wWynik  : WORD;

    // pętle
    i       : INT;
    iSuma   : DINT;
    iSilnia : DINT;

    // łańcuchy
    sImie   : STRING[20] := 'Jan';
    sNazw   : STRING[20] := 'Kowalski';
    sTekst  : STRING[80];
    iDlugosc: INT;

    // czas
    tA      : TIME := T#1h30m;
    tB      : TIME := T#45m;
    tWynik  : TIME;

    // instancje standardowych FB
    fbTON   : TON;
    fbCTU   : CTU;
    xWejscie: BOOL := TRUE;     // sygnał testowy dla timera
    xTakt   : BOOL;            // sygnał taktujący dla licznika

    // sterowanie pierwszym przebiegiem
    xInit   : BOOL := TRUE;    // TRUE tylko w 1. cyklu (część testów liczymy raz)
END_VAR

    // Funkcje konwersji (INT_TO_STRING, REAL_TO_STRING, BOOL_TO_STRING,
    // WORD_TO_STRING, DINT_TO_STRING, TIME_TO_STRING) pochodzą ze standardowej
    // biblioteki (CODESYS: Standard / TwinCAT: Tc2_Standard).

    (*───────────────────────────────────────────────────────────────────────────
      1) ARYTMETYKA  (liczone w każdym cyklu — wynik stały, więc widać go stale)
    ───────────────────────────────────────────────────────────────────────────*)
    iWynik := iA + iB;                              // oczekiwane: 5
    sLog01 := CONCAT('2 + 3 = ', INT_TO_STRING(iWynik));

    iWynik := iA * iB;                              // oczekiwane: 6
    sLog02 := CONCAT('2 * 3 = ', INT_TO_STRING(iWynik));

    iWynik := iB - iA;                              // oczekiwane: 1
    sLog03 := CONCAT('3 - 2 = ', INT_TO_STRING(iWynik));

    rWynik := rA / rB;                              // oczekiwane: 3.5
    sLog04 := CONCAT('7.0 / 2.0 = ', REAL_TO_STRING(rWynik));

    iWynik := 17 MOD 5;                             // oczekiwane: 2
    sLog05 := CONCAT('17 MOD 5 = ', INT_TO_STRING(iWynik));

    rWynik := EXPT(2.0, 10.0);                      // oczekiwane: 1024
    sLog06 := CONCAT('2 ** 10 = ', REAL_TO_STRING(rWynik));

    rWynik := SQRT(144.0);                          // oczekiwane: 12
    sLog07 := CONCAT('SQRT(144) = ', REAL_TO_STRING(rWynik));

    rWynik := ABS(-3.5);                            // oczekiwane: 3.5
    sLog08 := CONCAT('ABS(-3.5) = ', REAL_TO_STRING(rWynik));

    (*───────────────────────────────────────────────────────────────────────────
      2) PORÓWNANIA I LOGIKA  (wynik typu BOOL -> 'TRUE'/'FALSE')
    ───────────────────────────────────────────────────────────────────────────*)
    xWynik := (iA < iB);                            // oczekiwane: TRUE
    sLog09 := CONCAT('2 < 3 = ', BOOL_TO_STRING(xWynik));

    xWynik := xA AND xB;                            // oczekiwane: FALSE
    sLog10 := CONCAT('TRUE AND FALSE = ', BOOL_TO_STRING(xWynik));

    xWynik := xA OR xB;                             // oczekiwane: TRUE
    sLog11 := CONCAT('TRUE OR FALSE = ', BOOL_TO_STRING(xWynik));

    xWynik := xA XOR xB;                            // oczekiwane: TRUE
    sLog12 := CONCAT('TRUE XOR FALSE = ', BOOL_TO_STRING(xWynik));

    xWynik := NOT xA;                               // oczekiwane: FALSE
    sLog13 := CONCAT('NOT TRUE = ', BOOL_TO_STRING(xWynik));

    (*───────────────────────────────────────────────────────────────────────────
      3) OPERACJE BITOWE  (wA=16#000F=15, wB=16#00F0=240)
    ───────────────────────────────────────────────────────────────────────────*)
    wWynik := wA AND wB;                            // oczekiwane: 0
    sLog14 := CONCAT('15 AND 240 = ', WORD_TO_STRING(wWynik));

    wWynik := wA OR wB;                             // oczekiwane: 255
    sLog15 := CONCAT('15 OR 240 = ', WORD_TO_STRING(wWynik));

    wWynik := SHL(wA, 4);                           // oczekiwane: 240  (15<<4)
    sLog16 := CONCAT('15 SHL 4 = ', WORD_TO_STRING(wWynik));

    wWynik := SHR(wB, 4);                           // oczekiwane: 15   (240>>4)
    sLog17 := CONCAT('240 SHR 4 = ', WORD_TO_STRING(wWynik));

    (*───────────────────────────────────────────────────────────────────────────
      4) FUNKCJE WYBORU / GRANIC
    ───────────────────────────────────────────────────────────────────────────*)
    rWynik := MAX(5.0, 3.0);                        // oczekiwane: 5
    sLog18 := CONCAT('MAX(5,3) = ', REAL_TO_STRING(rWynik));

    rWynik := MIN(5.0, 3.0);                        // oczekiwane: 3
    sLog19 := CONCAT('MIN(5,3) = ', REAL_TO_STRING(rWynik));

    rWynik := LIMIT(0.0, 15.0, 10.0);               // oczekiwane: 10  (MN,IN,MX)
    sLog20 := CONCAT('LIMIT(0,15,10) = ', REAL_TO_STRING(rWynik));

    rWynik := SEL(TRUE, 1.0, 2.0);                  // oczekiwane: 2  (G=TRUE->IN1)
    sLog21 := CONCAT('SEL(TRUE,1,2) = ', REAL_TO_STRING(rWynik));

    (*───────────────────────────────────────────────────────────────────────────
      5) ŁAŃCUCHY ZNAKÓW
    ───────────────────────────────────────────────────────────────────────────*)
    sTekst := CONCAT(sImie, CONCAT(' ', sNazw));    // oczekiwane: 'Jan Kowalski'
    sLog22 := CONCAT('CONCAT = ', sTekst);

    iDlugosc := LEN(sTekst);                        // oczekiwane: 12
    sLog23 := CONCAT('LEN(Jan Kowalski) = ', INT_TO_STRING(iDlugosc));

    sTekst := LEFT(sTekst, 3);                      // oczekiwane: 'Jan'
    sLog24 := CONCAT('LEFT(...,3) = ', sTekst);

    (*───────────────────────────────────────────────────────────────────────────
      6) INSTRUKCJA CASE
    ───────────────────────────────────────────────────────────────────────────*)
    iWynik := 2;
    CASE iWynik OF
        1: sTekst := 'jeden';
        2: sTekst := 'dwa';
        3: sTekst := 'trzy';
    ELSE
        sTekst := 'inne';
    END_CASE
    sLog28 := CONCAT('CASE(2) = ', sTekst);         // oczekiwane: 'dwa'

    (*───────────────────────────────────────────────────────────────────────────
      7) PĘTLE  (liczone JEDEN RAZ — w pierwszym cyklu; wynik potem widoczny stale)
    ───────────────────────────────────────────────────────────────────────────*)
    IF xInit THEN
        // suma 1..10 (pętla FOR)
        iSuma := 0;
        FOR i := 1 TO 10 DO
            iSuma := iSuma + i;
        END_FOR
        sLog25 := CONCAT('suma 1..10 (FOR) = ', DINT_TO_STRING(iSuma));   // 55

        // silnia 5! (pętla FOR)
        iSilnia := 1;
        FOR i := 1 TO 5 DO
            iSilnia := iSilnia * i;
        END_FOR
        sLog26 := CONCAT('5! (FOR) = ', DINT_TO_STRING(iSilnia));         // 120

        // ile podwojeń aż wartość przekroczy 100 (pętla WHILE)
        diWynik := 0;
        iWynik  := 1;
        WHILE iWynik <= 100 DO
            iWynik  := iWynik * 2;     // 1->2->4->8->16->32->64->128
            diWynik := diWynik + 1;    // licznik kroków
        END_WHILE
        sLog27 := CONCAT('podwojen do >100 (WHILE) = ', DINT_TO_STRING(diWynik)); // 7

        xInit := FALSE;                // dalej już nie liczymy pętli
    END_IF

    (*───────────────────────────────────────────────────────────────────────────
      8) STANDARDOWE BLOKI FUNKCYJNE  (wywoływane w KAŻDYM cyklu — wartości żyją)
    ───────────────────────────────────────────────────────────────────────────*)
    // TON — opóźnienie załączenia: po 2 s ciągłego IN=TRUE, Q staje się TRUE.
    // Obserwuj ET rosnące do 2 s, potem Q = TRUE.
    fbTON(IN := xWejscie, PT := T#2s);
    sLog29 := CONCAT(CONCAT('TON: ET = ', TIME_TO_STRING(fbTON.ET)),
                     CONCAT('  Q = ', BOOL_TO_STRING(fbTON.Q)));

    // CTU — licznik w górę: xTakt przełącza się co cykl, więc CV stopniowo rośnie.
    // Q = TRUE gdy CV >= PV (=10). Obserwuj CV rosnące i Q.
    xTakt := NOT xTakt;
    fbCTU(CU := xTakt, RESET := FALSE, PV := 10);
    sLog30 := CONCAT(CONCAT('CTU: CV = ', TO_STRING(fbCTU.CV)),
                     CONCAT('  Q = ', BOOL_TO_STRING(fbCTU.Q)));

    (*───────────────────────────────────────────────────────────────────────────
      9) (OPCJONALNIE) WYPIS DO KONSOLI / DZIENNIKA
    ───────────────────────────────────────────────────────────────────────────*)
    // Jeśli chcesz realny "print" zamiast podglądu Watch:
    //  • CODESYS:  ADSLOGSTR(ADSLOG_MSGTYPE_HINT, '%s', sLog01);
    //  • TwinCAT:  ADSLOGSTR(ADSLOG_MSGTYPE_HINT, '%s', sLog01);
    //  • SoMachine/inne: funkcje logowania zależne od platformy.
    // Wymaga dołączenia odpowiedniej biblioteki (np. Tc2_System w TwinCAT).

END_PROGRAM
