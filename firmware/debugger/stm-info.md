## 🧠 1) STM32F746NGH6-muistin yleiskuva

**Flash**

* Laite sisältää **1 MByte sisäistä Flash-muistia**, joka sijaitsee flash-aluessa osoitteen **0x0800 0000** alkaen.
  Tämä on se alue, johon sovellusfirmwaresi tyypillisesti linkitetään ja josta MCU boottaa. ([STMicroelectronics][1])

**SRAM / RAM**

* MCU:ssa on yhteensä **n. 320 KB sisäistä SRAM-muistia** käytettäväksi ohjelman dynaamisille muuttujille. ([STMicroelectronics][1])
* STM32F7-perheessä (mukaan lukien F746) RAM ei ole yksi kokonainen yhtenäinen lohko, vaan se koostuu useammasta erillisestä segmentistä: **ITCM-RAM, DTCM-RAM, SRAM1 ja SRAM2** — eri tarkoituksiin ja erilaisilla suorituskyky- ja pääsyominaisuuksilla. ([STMicroelectronics][2])

---

## 🧱 2) Mitkä RAM-alueet STM32F7 tukee

Yleisesti STM32F7-sarjassa:

**ITCM-RAM**

* Tightly Coupled Memory — ns. “koodin lähellä” oleva RAM, josta CPU voi hakea dataa erittäin nopeasti ilman välimuistia.
* Usein käytetään esimerkiksi ISR-pinnoille tai erittäin deterministisiin viime hetken laskutoimituksiin.
* Tyypillinen alkuosoite on **0x0000 0000**. ([Scribd][3])

**DTCM-RAM**

* Myös TCM-tyyppinen RAM, mutta dataankäyttöön (eri kuin ITCM, joka on tarkoitettu instruktioille).
* Tyypillinen alkuosoite on **0x2000 0000**. ([Scribd][3])

**SRAM1 ja SRAM2**

* **SRAM1** aloittaa usein osoitteesta **0x2001 0000** ja on osa “normaalia” RAM-aluetta CPU, bus-maisterit ja DMA voivat käyttää sitä.
* **SRAM2** aloittaa korkeammalta, esimerkiksi **0x2004 C000** (osoite alueen hahmottamiseksi) ja sisältää lisämuistia sys-dataan. ([Scribd][3])

> **Muistin osoitteet eivät ala vain ”0x2000 0000” ilman kontekstia** — STM32F7-perheessä RAM on segmentoitunut eikä yksi yhtenäinen alue. Tämä on tärkeää linker-skriptiä mietittäessä. ([Scribd][3])

---

## 📌 3) Miten tämä liittyy Rust-projektisi linker-skriptiin

Koska STM32F746NGH6:ssa RAM-alue ei ole yksi jatkuva lohko alkaen 0x2000 0000:

❌ Tämä on **väärin**:

```
RAM : ORIGIN = 0x20010000, LENGTH = 0x00040000
```

Tämä arvo ei vastaa muistialueen todellista kartoitusta, koska:

* se alkaa keskeltä RAM-aluetta
* ohittaa kokonaisen DTCM-osion
* voi johtaa **virheellisiin stack/heap-osoitteisiin**
* saattaa sijoittaa RTT-bufferin epäsopivaan muistialueeseen

Oikea tapa on käyttää **RAMia alkaen 0x2000 0000**, koska se on koko RAMin “low-level” alku piste, eikä vain SRAM1 tai SRAM2 erillinen lohko. ([STMicroelectronics][2])

---

## 📈 4) Suositeltava MEMORY-lohko linker-skriptiin

Nykyiselle RAM-kokoonpanolle (320 KB):

```
MEMORY
{
  FLASH : ORIGIN = 0x08000000, LENGTH = 1024K
  RAM :   ORIGIN = 0x20000000, LENGTH = 320K
}
```

**Miksi tämä toimii:**

* koko sisäinen RAM on mukana
* stack/heap eivät päädy “kerrottuja” segmenttejä rikkoviin alueisiin
* RTT ja muut globaalit data-osiot saavat varmasti laillisen muistialueen

Tämä on käytännössä se malli, jota ST:n oma HAL ja Cube tukevat oletuksena. ([STMicroelectronics][1])

---

## ⚠️ 5) Mikä osa RAMista on kriittinen revisioissa

### 🧠 DTCM vs. SRAM

* DTCM-RAM (0x20000000 alkaen) on erittäin nopeaa, mutta sillä ei ole cachea ja se saattaa olla eri pääsyn reitillä kuin muut RAM-alueet. ([STMicroelectronics][2])

### 🧩 SRAM1 ja SRAM2

* SRAM1 ja SRAM2 ovat “normaalia RAMia”, joita käytetään yleisesti globaalien muuttujien, heapin ja stackin kanssa. ([Scribd][3])

Jos sijoitat dataa **eri RAM-lohkoihin** (esim. ITCM vs SRAM), sinun täytyy tietää, mikä lohko on cacheable tai ei, koska esimerkiksi DMA-laitteet eivät välttämättä voi käyttää kaikkia alueita ilman MPU-asetuksia.

---

## 🧠 6) Yhteenveto dokumentointiin

📌 **STM32F746NGH6 sisältää:**

* 1 MByte Flash (0x0800_0000 alkaen)
* ~320 KB sisäistä RAMia
* RAM on pilkottu eri lohkoihin: DTCM, SRAM1, SRAM2, ITCM jne. ([STMicroelectronics][2])

📌 **Linkker-skriptin RAM-osoite tulisi olla 0x2000 0000**, ei 0x2001 0000.

📌 **ITCM- ja DTCM-muisti voivat olla soveltuvia erityisiin käyttötarkoituksiin** kuten stack-tms. realtime data, mutta vaativat erillistä huomiointia (cache ym.).

📌 **STM32RAM-kartoitus on monimutkaisempi kuin perus STM32F4**, koska F7-sarjassa on useita eri RAM-lohkoja. ([Scribd][3])
