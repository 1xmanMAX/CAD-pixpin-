//! Coordenadas en **pixeles fisicos** del escritorio virtual.
//!
//! Todo en este crate trabaja en pixeles fisicos, nunca en unidades
//! independientes del dispositivo. Con DPI mixto —un portatil al 150% junto a
//! un monitor externo al 100%— mezclar ambos sistemas es la via mas rapida a
//! una lupa borrosa y a bordes desalineados. La conversion se hace una sola
//! vez, en la frontera con Win32.

/// Un punto del escritorio virtual. Puede ser negativo: el monitor principal
/// esta en el origen y los demas se colocan alrededor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Punto {
    pub x: i32,
    pub y: i32,
}
